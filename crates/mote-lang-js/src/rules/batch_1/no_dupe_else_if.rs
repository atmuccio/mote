//! `js/no-dupe-else-if`

use super::util;
use crate::prelude::*;

pub struct NoDupeElseIf;

pub static NO_DUPE_ELSE_IF: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["suspicious"],
    remediation_minutes: 5,
    description: r#"Flags `else if` conditions that can never be true because an earlier condition of the same
`if … else if` chain already covers them.

**Why:** a branch of the chain only runs when every earlier condition was false. If an earlier
condition is the same test (or a weaker one, like `a` before `a && b`, or `a || b` before `b`), the
later branch is dead code — usually a copy-paste error where the condition was not updated.

**Fix:** correct the duplicated condition, or merge the branches.

```js
if (kind === 'a') {
  handleA();
} else if (kind === 'b') {
  handleB();
} else if (kind === 'a') {   // flagged: already handled above
  handleC();
}
```

Only conditions without side effects are compared (no calls, assignments, or `await`), and a
condition with side effects in between ends the comparison, so a chain that deliberately
re-evaluates state is not reported.
"#,
    ..RuleMeta::new(
        "js/no-dupe-else-if",
        "js",
        "Conditions in an if-else-if chain should not be duplicated",
    )
};

impl JsRule for NoDupeElseIf {
    fn meta(&self) -> &'static RuleMeta {
        &NO_DUPE_ELSE_IF
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::IfStatement]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::IfStatement(head) = node.kind() else {
            return;
        };
        let Some(Statement::IfStatement(_)) = &head.alternate else {
            return;
        };
        // Only the head of a chain; the `else if` links are visited from here.
        if matches!(ctx.parent_kind(node), Some(AstKind::IfStatement(p))
            if p.alternate.as_ref().is_some_and(|a| a.span() == head.span))
        {
            return;
        }

        // Each pure condition in disjunctive form: `||` operands, each a list of `&&` operands.
        let mut earlier: Vec<(Span, Vec<Vec<&Expression<'a>>>)> = Vec::new();
        let mut test = &head.test;
        let mut next = &head.alternate;
        loop {
            if util::is_pure(test) {
                let form = disjunctive_form(test);
                // The nearest earlier conditions only: comparing every link of a machine-made
                // chain of thousands with every earlier one was quadratic.
                let window = &earlier[earlier.len().saturating_sub(MAX_EARLIER)..];
                if let Some((covering, _)) = window.iter().find(|(_, e)| covers(e, &form)) {
                    let covering = *covering;
                    let shown = util::excerpt(ctx.text(test.span()));
                    ctx.emit(
                        test.span(),
                        format!(
                            "Change this condition: '{shown}' is already handled by an earlier branch"
                        ),
                    )
                    .secondary(covering, "Earlier condition");
                }
                earlier.push((test.span(), form));
            } else {
                // A call or assignment may change what later conditions see.
                earlier.clear();
            }
            match next {
                Some(Statement::IfStatement(link)) => {
                    test = &link.test;
                    next = &link.alternate;
                }
                _ => break,
            }
        }
    }
}

/// Earlier conditions of a chain each condition is compared with.
const MAX_EARLIER: usize = 256;

fn disjunctive_form<'b, 'a>(expr: &'b Expression<'a>) -> Vec<Vec<&'b Expression<'a>>> {
    let mut or = Vec::new();
    split(expr, LogicalOperator::Or, &mut or);
    or.into_iter()
        .map(|d| {
            let mut and = Vec::new();
            split(d, LogicalOperator::And, &mut and);
            and
        })
        .collect()
}

/// Does `earlier` being false guarantee that `later` is false? True when every `||` operand of
/// `later` has, among the `||` operands of `earlier`, one whose `&&` operands all appear in it.
fn covers(earlier: &[Vec<&Expression<'_>>], later: &[Vec<&Expression<'_>>]) -> bool {
    later.iter().all(|l_and| {
        earlier.iter().any(|e_and| {
            e_and
                .iter()
                .all(|ea| l_and.iter().any(|la| util::same_pure_expr(ea, la)))
        })
    })
}

/// Flatten nested `op` operands (`a || (b || c)` → `[a, b, c]`).
fn split<'b, 'a>(expr: &'b Expression<'a>, op: LogicalOperator, out: &mut Vec<&'b Expression<'a>>) {
    match expr {
        Expression::LogicalExpression(l) if l.operator == op => {
            split(&l.left, op, out);
            split(&l.right, op, out);
        }
        _ => out.push(expr),
    }
}
