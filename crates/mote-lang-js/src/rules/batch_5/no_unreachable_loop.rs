//! `js/no-unreachable-loop`

use crate::prelude::*;

pub struct NoUnreachableLoop;

pub static NO_UNREACHABLE_LOOP: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["suspicious", "loops"],
    remediation_minutes: 5,
    description: r#"Flags `for`, `while`, and `do … while` loops whose body always leaves the loop during the first
iteration, through `return`, `throw`, or `break`.

**Why:** such a loop runs its body at most once, so it is not a loop at all. Usually an exit that
should be conditional is not: the `return` meant for "found it" sits outside the `if`, or a
`break` was added for debugging and left in place. Every element after the first is silently
ignored.

**Fix:** move the exit into the condition it belongs to, or replace the loop with an `if` if a
single pass is really intended.

```js
for (let i = 0; i < items.length; i++) {
  if (items[i].id === id) {
    found = items[i];
  }
  break;                       // flagged loop: only items[0] is ever checked
}

for (let i = 0; i < items.length; i++) {
  if (items[i].id === id) {
    found = items[i];
    break;                     // compliant
  }
}
```

A `continue` anywhere in the body means another iteration is possible, so such loops are not
reported. `for…of` and `for…in` loops are not reported either: returning from the first iteration
is a common way to take the first element of an iterable or to test whether an object has any
key. `do { … } while (false)` blocks are not loops by intent and are skipped too, and so is an exit
with a comment right before it or on its line, which marks it as deliberate (`break; // only the
first case while debugging`).
"#,
    ..RuleMeta::new(
        "js/no-unreachable-loop",
        "js",
        "Loops should be able to run more than one iteration",
    )
};

impl JsRule for NoUnreachableLoop {
    fn meta(&self) -> &'static RuleMeta {
        &NO_UNREACHABLE_LOOP
    }

    fn types(&self) -> &'static [AstType] {
        &[
            AstType::ForStatement,
            AstType::WhileStatement,
            AstType::DoWhileStatement,
        ]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let (body, keyword) = match node.kind() {
            AstKind::ForStatement(f) => (&f.body, "for"),
            AstKind::WhileStatement(w) => (&w.body, "while"),
            AstKind::DoWhileStatement(d) => {
                if is_falsy_constant(&d.test) {
                    return;
                }
                (&d.body, "do")
            }
            _ => return,
        };
        let label = match ctx.parent_kind(node) {
            Some(AstKind::LabeledStatement(l)) => Some(l.label.name.as_str()),
            _ => None,
        };
        if !exits(body, label) || has_continue(body, label, true) {
            return;
        }
        if explained_exit(ctx, body, label) {
            return;
        }
        let span = Span::new(node.span().start, node.span().start + keyword.len() as u32);
        ctx.emit(
            span,
            "This loop always exits during its first iteration: make the exit conditional or use an 'if'",
        );
    }
}

fn is_falsy_constant(expr: &Expression<'_>) -> bool {
    match helpers::unwrap_expr(expr) {
        Expression::BooleanLiteral(b) => !b.value,
        Expression::NumericLiteral(n) => n.value == 0.0,
        _ => false,
    }
}

/// Does every path through the statement leave the loop (`return`, `throw`, or a `break` of the
/// loop itself)?
fn exits(stmt: &Statement<'_>, label: Option<&str>) -> bool {
    match stmt {
        Statement::ReturnStatement(_) | Statement::ThrowStatement(_) => true,
        Statement::BreakStatement(b) => match &b.label {
            None => true,
            Some(l) => Some(l.name.as_str()) == label,
        },
        Statement::BlockStatement(b) => b.body.iter().any(|s| exits(s, label)),
        Statement::IfStatement(i) => {
            exits(&i.consequent, label) && i.alternate.as_ref().is_some_and(|a| exits(a, label))
        }
        _ => false,
    }
}

/// Is there a `continue` that targets the loop (an unlabeled one outside nested loops, or one with
/// the loop's label anywhere)? Nested functions are not searched.
fn has_continue(stmt: &Statement<'_>, label: Option<&str>, own: bool) -> bool {
    let nested = |s: &Statement<'_>| has_continue(s, label, own);
    match stmt {
        Statement::ContinueStatement(c) => match &c.label {
            None => own,
            Some(l) => Some(l.name.as_str()) == label,
        },
        Statement::BlockStatement(b) => b.body.iter().any(nested),
        Statement::IfStatement(i) => {
            nested(&i.consequent) || i.alternate.as_ref().is_some_and(nested)
        }
        Statement::LabeledStatement(l) => nested(&l.body),
        Statement::TryStatement(t) => {
            t.block.body.iter().any(nested)
                || t.handler
                    .as_ref()
                    .is_some_and(|h| h.body.body.iter().any(nested))
                || t.finalizer
                    .as_ref()
                    .is_some_and(|f| f.body.iter().any(nested))
        }
        Statement::SwitchStatement(s) => s.cases.iter().any(|c| c.consequent.iter().any(nested)),
        Statement::ForStatement(f) => has_continue(&f.body, label, false),
        Statement::ForInStatement(f) => has_continue(&f.body, label, false),
        Statement::ForOfStatement(f) => has_continue(&f.body, label, false),
        Statement::WhileStatement(w) => has_continue(&w.body, label, false),
        Statement::DoWhileStatement(d) => has_continue(&d.body, label, false),
        Statement::WithStatement(w) => nested(&w.body),
        _ => false,
    }
}

/// Is there a comment just before the statement that ends the first iteration (or on its line)?
/// Such an exit is deliberate: `break; // remove to test every combination`.
fn explained_exit(ctx: &Ctx<'_>, body: &Statement<'_>, label: Option<&str>) -> bool {
    let (list, start): (&[Statement<'_>], u32) = match body {
        Statement::BlockStatement(b) => (&b.body, b.span.start),
        other => (std::slice::from_ref(other), other.span().start),
    };
    let mut previous_end = start;
    for stmt in list {
        if exits(stmt, label) {
            let end = stmt.span().end;
            let line_end = ctx
                .source()
                .get(end as usize..)
                .and_then(|rest| rest.find('\n'))
                .map_or(end, |n| end + n as u32);
            // Own-line comments after the previous statement, or comments on the exit's own line; not a
            // trailing comment of the previous line (such as one after the loop's opening brace).
            let after_line = ctx.line_of(previous_end);
            let comments = ctx.comments();
            let first = comments.partition_point(|c| c.span.start < previous_end);
            return comments
                .get(first..)
                .unwrap_or(&[])
                .iter()
                .take_while(|c| c.span.start < line_end)
                .any(|c| ctx.line_of(c.span.start) > after_line);
        }
        previous_end = stmt.span().end;
    }
    false
}
