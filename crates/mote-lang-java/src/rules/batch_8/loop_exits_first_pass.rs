//! `java/loop-exits-first-pass` — a loop whose body always leaves the loop on its first pass.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["loops", "suspicious"],
    remediation_minutes: 5,
    description: r#"Detects loops (`for`, enhanced `for`, `while`, `do`) whose body ends with an
unconditional `break`, `return` or `throw` and contains no `continue` for that loop, so the
body can never run a second time.

**Why it matters.** Every path through such a body leaves the loop: either it reaches the
final jump, or it exits earlier through another `break`, `return` or exception. The update
clause never runs and the condition is tested once. Either the loop is an `if` in disguise,
which misleads the reader into looking for the iteration, or the final jump was meant to be
conditional (placed one brace too low, or left over from debugging) and the loop silently
processes only its first element.

**How to fix.** If one pass is intended, write it as an `if` (for the first element of a
collection: `if (!items.isEmpty()) … items.get(0)`, or `iterator().next()` after a check). If
the loop should go on, make the jump conditional.

```java
// Noncompliant
for (Order order : orders) {
    if (order.isOpen()) {
        close(order);
    }
    break;              // only the first order is ever looked at
}

// Compliant
for (Order order : orders) {
    if (order.isOpen()) {
        close(order);
        break;
    }
}
```

Not reported: loops with a `continue` that targets them (search and retry loops, where the
final jump is the normal exit after the retries), `do { … } while (false)` blocks, and jumps
that are not the last statement of the body."#,
    ..RuleMeta::new(
        "java/loop-exits-first-pass",
        "java",
        "Loop body always exits on the first pass",
    )
};

pub struct LoopExitsFirstPass;

/// The last statement of a loop body (the body itself when it is not a block).
fn last_statement(body: Node<'_>) -> Option<Node<'_>> {
    if kind(body) == "block" {
        named_children(body).last()
    } else {
        Some(body)
    }
}

impl<'t> JavaRule<'t> for LoopExitsFirstPass {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "for_statement",
            "enhanced_for_statement",
            "while_statement",
            "do_statement",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        let Some(last) = last_statement(body) else {
            return;
        };
        let what = match kind(last) {
            "break_statement" => "break",
            "return_statement" => "return",
            "throw_statement" => "throw",
            _ => return,
        };
        if kind(node) == "do_statement"
            && node
                .field(Field::Condition)
                .map(syntax::strip_parens)
                .is_some_and(|c| kind(c) == "false")
        {
            return;
        }
        // A `continue` that targets this loop lets it go on.
        let mut continues = Vec::new();
        let broken = util::visit(body, |n| match kind(n) {
            "continue_statement" => {
                continues.push(n);
                util::Walk::Skip
            }
            "ERROR" => util::Walk::Stop,
            _ if util::is_nested_scope(n) => util::Walk::Skip,
            _ => util::Walk::Continue,
        });
        if broken
            || continues
                .iter()
                .any(|c| util::jump_target(ctx, *c).is_none_or(|t| t.id() == node.id()))
        {
            return;
        }
        let msg = match kind(node) {
            "enhanced_for_statement" => format!(
                "This loop never looks past the first element: its body always ends with `{what}`"
            ),
            _ => format!("This loop never runs a second pass: its body always ends with `{what}`"),
        };
        ctx.emit(last, msg);
    }
}
