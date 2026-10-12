//! `js/no-infinite-loop`

use super::util::is_function_node;
use crate::prelude::*;
use crate::rules::batch_6::util::descendants;

pub struct NoInfiniteLoop;

pub static NO_INFINITE_LOOP: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["suspicious"],
    remediation_minutes: 10,
    description: r#"Flags a `while (true)`, `do … while (true)` or `for (;;)` loop that contains no way out: no
`break` or `continue` that leaves it, no `return`, no `throw`, and no call to `process.exit`.

**Why:** a loop whose condition is always true only ends through a statement inside it. When there
is none, the loop spins forever and — JavaScript being single-threaded — freezes the page or the
server with it. The usual cause is a forgotten `break`, a `continue` written where a `break` was
meant, or an exit condition that was refactored away.

**Fix:** add the missing exit (`break`, `return`, `throw`), or write the real stop condition in the
loop header.

```js
while (true) {                  // flagged: nothing leaves the loop
  const job = queue.shift();
  if (!job) continue;
  run(job);
}

while (true) {                  // compliant
  const job = queue.shift();
  if (!job) break;
  run(job);
}
```

Deliberate endless loops are left alone: loops in generators (they pause at each `yield`), loops
that `await` (polling or worker loops that yield to the event loop), and loops inside a
`try … catch` of the same function (they end by an exception thrown from a call). Unlike
`js/no-constant-condition` with `check_loops`, which reports every constant loop condition, this
rule only reports loops that cannot end.
"#,
    ..RuleMeta::new(
        "js/no-infinite-loop",
        "js",
        "Loops with a constant true condition should have a reachable exit",
    )
};

impl JsRule for NoInfiniteLoop {
    fn meta(&self) -> &'static RuleMeta {
        &NO_INFINITE_LOOP
    }

    fn types(&self) -> &'static [AstType] {
        &[
            AstType::WhileStatement,
            AstType::DoWhileStatement,
            AstType::ForStatement,
        ]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let (always_true, keyword) = match node.kind() {
            AstKind::WhileStatement(s) => (is_true(&s.test), "while"),
            AstKind::DoWhileStatement(s) => (is_true(&s.test), "do"),
            AstKind::ForStatement(s) => (s.test.as_ref().is_none_or(is_true), "for"),
            _ => return,
        };
        if !always_true {
            return;
        }
        if in_generator_or_try(ctx, node) {
            return;
        }
        let own_label = match ctx.parent_kind(node) {
            Some(AstKind::LabeledStatement(l)) => Some(l.label.name.as_str()),
            _ => None,
        };
        let loop_span = node.span();
        // Nodes inside nested functions end before this offset.
        let mut skip_until = 0;
        for inner in descendants(ctx, node) {
            let span = inner.span();
            if span.start < skip_until {
                continue;
            }
            let kind = inner.kind();
            if is_function_node(kind) {
                skip_until = span.end;
                continue;
            }
            let exits = match kind {
                AstKind::ReturnStatement(_)
                | AstKind::ThrowStatement(_)
                | AstKind::YieldExpression(_)
                | AstKind::AwaitExpression(_) => true,
                AstKind::ForOfStatement(f) => f.r#await,
                AstKind::BreakStatement(b) => leaves_loop(
                    ctx,
                    inner,
                    node.id(),
                    b.label.as_ref().map(|l| l.name.as_str()),
                    true,
                ),
                AstKind::ContinueStatement(c) => match &c.label {
                    Some(label) if Some(label.name.as_str()) != own_label => {
                        leaves_loop(ctx, inner, node.id(), Some(label.name.as_str()), false)
                    }
                    _ => false,
                },
                AstKind::CallExpression(call) => is_exit_call(&call.callee),
                _ => false,
            };
            if exits {
                return;
            }
        }
        ctx.emit(
            Span::new(loop_span.start, loop_span.start + keyword.len() as u32),
            "This loop can never end: nothing in it breaks out, returns or throws; add the missing exit or a real loop condition",
        );
    }
}

/// `true`, `1` or another truthy numeric literal, `!0`, `!false`.
fn is_true(expr: &Expression<'_>) -> bool {
    match helpers::unwrap_expr(expr) {
        Expression::BooleanLiteral(b) => b.value,
        Expression::NumericLiteral(n) => n.value != 0.0 && !n.value.is_nan(),
        Expression::UnaryExpression(u) if u.operator == UnaryOperator::LogicalNot => {
            matches!(
                helpers::unwrap_expr(&u.argument),
                Expression::BooleanLiteral(b) if !b.value
            ) || matches!(
                helpers::unwrap_expr(&u.argument),
                Expression::NumericLiteral(n) if n.value == 0.0
            )
        }
        _ => false,
    }
}

/// The loop runs in a generator (it pauses at each `yield`), or inside a `try` block with a
/// `catch` in the same function (an exception from a call is the way out).
fn in_generator_or_try(ctx: &Ctx<'_>, node: &AstNode<'_>) -> bool {
    let at = node.span().start;
    for ancestor in ctx.ancestors(node) {
        match ancestor.kind() {
            AstKind::Function(f) => return f.generator,
            AstKind::ArrowFunctionExpression(_) => return false,
            AstKind::TryStatement(t)
                if t.handler.is_some() && t.block.span.start <= at && at < t.block.span.end =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}

/// Does this `break` (or labelled `continue`) transfer control out of the loop `loop_id`?
/// An unlabelled `break` leaves the loop unless a nearer loop or `switch` catches it; a labelled
/// one leaves it unless the label belongs to a statement inside the loop.
fn leaves_loop(
    ctx: &Ctx<'_>,
    jump: &AstNode<'_>,
    loop_id: NodeId,
    label: Option<&str>,
    is_break: bool,
) -> bool {
    for ancestor in ctx.ancestors(jump) {
        if ancestor.id() == loop_id {
            return true;
        }
        match (label, ancestor.kind()) {
            (Some(name), AstKind::LabeledStatement(l)) if l.label.name == name => return false,
            (None, kind)
                if is_break
                    && (helpers::is_loop(kind) || matches!(kind, AstKind::SwitchStatement(_))) =>
            {
                return false;
            }
            _ => {}
        }
    }
    false
}

/// `process.exit(…)`, `Deno.exit(…)`.
fn is_exit_call(callee: &Expression<'_>) -> bool {
    matches!(
        helpers::static_member_names(helpers::unwrap_expr(callee)),
        Some(("process" | "Deno", "exit"))
    )
}
