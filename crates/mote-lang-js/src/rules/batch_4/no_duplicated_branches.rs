//! `js/no-duplicated-branches`

use super::util;
use crate::prelude::*;
use oxc_span::ContentEq;

pub struct NoDuplicatedBranches;

pub static NO_DUPLICATED_BRANCHES: RuleMeta = RuleMeta {
    severity: Severity::Medium,
    tags: &["duplication", "suspicious"],
    remediation_minutes: 10,
    description: r#"Flags branches of an `if` / `else if` chain, and cases of a `switch`, whose bodies are
identical to an earlier branch.

**Why:** two conditions leading to exactly the same code are either a copy-paste slip (one branch
was meant to be edited after pasting) or needless repetition that hides the fact that the
conditions share an outcome.

**Fix:** if the duplication is intended, merge the branches: combine the conditions with `||`, or
stack the `case` labels. Otherwise, correct the branch that was meant to differ.

```js
if (kind === 'a') {           // compliant: first occurrence
  prepare();
  render(item);
} else if (kind === 'b') {    // flagged: same body as the first branch
  prepare();
  render(item);
}

switch (kind) {
  case 'a':
  case 'b':                   // compliant: labels stacked
    prepare();
    render(item);
    break;
}
```

Only non-trivial bodies are compared: at least two statements (a closing `break` does not count),
or one statement spread over several lines. A `case` that falls through into the next one is never
compared.
"#,
    ..RuleMeta::new(
        "js/no-duplicated-branches",
        "js",
        "Branches should not have identical bodies",
    )
};

impl JsRule for NoDuplicatedBranches {
    fn meta(&self) -> &'static RuleMeta {
        &NO_DUPLICATED_BRANCHES
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::IfStatement, AstType::SwitchStatement]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        match node.kind() {
            AstKind::IfStatement(stmt) => check_if_chain(ctx, node, stmt),
            AstKind::SwitchStatement(stmt) => check_switch(ctx, stmt),
            _ => {}
        }
    }
}

/// One branch: its statements and the span to report it at.
struct Branch<'b, 'a> {
    body: &'b [Statement<'a>],
    at: Span,
}

fn body_of<'b, 'a>(stmt: &'b Statement<'a>) -> &'b [Statement<'a>] {
    match stmt {
        Statement::BlockStatement(b) => &b.body,
        other => std::slice::from_ref(other),
    }
}

fn check_if_chain<'a>(ctx: &mut Ctx<'a>, node: &AstNode<'a>, head: &'a IfStatement<'a>) {
    // A lone `if` has one branch: nothing to compare.
    if head.alternate.is_none() {
        return;
    }
    // Analyze each chain once, from its head.
    if let Some(AstKind::IfStatement(parent)) = ctx.parent_kind(node) {
        if parent
            .alternate
            .as_ref()
            .is_some_and(|alt| alt.span() == head.span)
        {
            return;
        }
    }
    let mut branches: Vec<Branch<'_, 'a>> = vec![Branch {
        body: body_of(&head.consequent),
        at: util::first_line(ctx, head.test.span()),
    }];
    let mut current = head;
    loop {
        match &current.alternate {
            Some(Statement::IfStatement(next)) => {
                branches.push(Branch {
                    body: body_of(&next.consequent),
                    at: util::first_line(ctx, next.test.span()),
                });
                current = next;
            }
            Some(other) => {
                let gap = Span::new(current.consequent.span().end, other.span().start);
                let at = match ctx.text(gap).find("else") {
                    Some(i) => Span::new(gap.start + i as u32, gap.start + i as u32 + 4),
                    None => util::first_line(ctx, other.span()),
                };
                branches.push(Branch {
                    body: body_of(other),
                    at,
                });
                break;
            }
            None => break,
        }
    }
    report_duplicates(ctx, &branches, "branch");
}

fn check_switch<'a>(ctx: &mut Ctx<'a>, stmt: &'a SwitchStatement<'a>) {
    let last = stmt.cases.len().saturating_sub(1);
    let mut branches: Vec<Branch<'_, 'a>> = Vec::new();
    for (i, case) in stmt.cases.iter().enumerate() {
        let Some(tail) = case.consequent.last() else {
            continue; // stacked label
        };
        // A case that falls through runs the next case's code too: its body is not self-contained.
        if i != last && !util::always_exits(tail) {
            continue;
        }
        let body = match tail {
            Statement::BreakStatement(b) if b.label.is_none() => {
                &case.consequent[..case.consequent.len() - 1]
            }
            _ => &case.consequent[..],
        };
        let at = match &case.test {
            Some(test) => util::first_line(ctx, test.span()),
            None => util::keyword_span(ctx, case.span, "default"),
        };
        branches.push(Branch { body, at });
    }
    report_duplicates(ctx, &branches, "case");
}

fn is_non_trivial(ctx: &Ctx<'_>, body: &[Statement<'_>]) -> bool {
    match body {
        [] => false,
        [one] => {
            let span = one.span();
            ctx.line_of(span.start) != ctx.line_of(span.end.saturating_sub(1))
        }
        _ => true,
    }
}

fn same_body(a: &[Statement<'_>], b: &[Statement<'_>]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.content_eq(y))
}

fn report_duplicates(ctx: &mut Ctx<'_>, branches: &[Branch<'_, '_>], noun: &str) {
    for (i, branch) in branches.iter().enumerate().skip(1) {
        if !is_non_trivial(ctx, branch.body) {
            continue;
        }
        let Some(original) = branches[..i]
            .iter()
            .find(|earlier| same_body(earlier.body, branch.body))
        else {
            continue;
        };
        let line = ctx.line_of(original.at.start);
        ctx.emit(
            branch.at,
            format!(
                "Merge this {noun} with the identical {noun} on line {line}, or change one of them"
            ),
        )
        .secondary(original.at, format!("Identical {noun}"));
    }
}
