//! `java/empty-block` — empty control-flow blocks, initializers, and nested blocks.

use super::util::{self, Fld, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Medium,
    tags: &["unused", "suspicious"],
    remediation_minutes: 5,
    description: r#"Detects blocks with no statements and no comment: the body of an `if`,
`else`, loop, `try`, `finally`, `synchronized`, or `switch`, a static or instance initializer,
and a nested `{ }` block.

**Why it matters.** An empty block is either unfinished code (a forgotten implementation, an
`if` whose action was deleted) or a condition written the wrong way round (`if (ok) {} else
{ fail(); }`). Either way the reader has to guess what was intended.

**How to fix.** Add the missing code, invert the condition, or remove the block. If doing
nothing really is correct, say why in a comment inside the block.

```java
// Noncompliant
if (cache.isValid()) {
} else {
    cache.reload();
}

// Compliant
if (!cache.isValid()) {
    cache.reload();
}
```

Not reported: `catch` blocks (see `empty-catch-block`) and an empty `try` whose catch blocks
are all empty too (that no-op statement is already reported there), method, constructor and
lambda bodies,
arrow-switch cases `case X -> {}`, try-with-resources bodies (acquiring and releasing the
resource is the point), and loops that do their work in the header — a `while`/`do` whose
condition calls a method (`while (queue.poll() != null) {}`) or a `for` whose counter outlives
the loop (`for (i = 0; a[i] != 0; i++) {}`)."#,
    ..RuleMeta::new("java/empty-block", "java", "Empty block")
};

pub struct EmptyBlock;

/// No statement and no comment (comments are named children too). O(1).
fn is_empty(block: Node<'_>) -> bool {
    block.named_child_count() == 0
}

impl<'t> JavaRule<'t> for EmptyBlock {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["block", "switch_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if kind(node) == "switch_expression" {
            let Some(body) = node.fld(F::Body) else {
                return;
            };
            if is_empty(body) && !body.has_error() {
                let at = util::keyword(node, "switch").unwrap_or(node);
                ctx.emit(at, "Remove this empty `switch` or add the missing cases");
            }
            return;
        }
        if !is_empty(node) {
            return;
        }
        let Some(parent) = ctx.parent(node) else {
            return;
        };
        let is_field = |f: F| parent.fld(f).is_some_and(|c| c.id() == node.id());
        let (at, what) = match kind(parent) {
            "if_statement" => {
                if is_field(F::Consequence) {
                    (util::keyword(parent, "if"), "if")
                } else if is_field(F::Alternative) {
                    (util::keyword(parent, "else"), "else")
                } else {
                    return;
                }
            }
            "for_statement" | "enhanced_for_statement" | "while_statement" | "do_statement" => {
                if !is_field(F::Body) || util::loop_header_does_work(parent) {
                    return;
                }
                let kw = match kind(parent) {
                    "while_statement" => "while",
                    "do_statement" => "do",
                    _ => "for",
                };
                (util::keyword(parent, kw), kw)
            }
            "try_statement" => {
                if !is_field(F::Body) {
                    return;
                }
                // `try {} catch (E e) {}` does nothing at all: one finding (the empty catch,
                // from `empty-catch-block`) is enough.
                let mut catches = named_children(parent)
                    .filter(|c| kind(*c) == "catch_clause")
                    .peekable();
                if catches.peek().is_some()
                    && catches.all(|c| c.fld(F::Body).is_none_or(syntax::is_empty_block))
                {
                    return;
                }
                (util::keyword(parent, "try"), "try")
            }
            "finally_clause" => (util::keyword(parent, "finally"), "finally"),
            "synchronized_statement" => (util::keyword(parent, "synchronized"), "synchronized"),
            "static_initializer" => (util::keyword(parent, "static"), "static initializer"),
            "class_body" | "enum_body_declarations" => (Some(node), "initializer"),
            "block" | "constructor_body" | "switch_block_statement_group" => (Some(node), "nested"),
            _ => return,
        };
        let at = at.unwrap_or(node);
        let msg = match what {
            "initializer" => {
                "Remove this empty initializer block or add a comment explaining it".to_string()
            }
            "static initializer" => {
                "Remove this empty static initializer or add a comment explaining it".to_string()
            }
            "nested" => "Remove this empty block".to_string(),
            kw => format!(
                "Remove this empty `{kw}` block or add a comment explaining why nothing is done"
            ),
        };
        ctx.emit(at, msg);
    }
}
