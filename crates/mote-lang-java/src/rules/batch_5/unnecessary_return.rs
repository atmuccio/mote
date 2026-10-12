//! `java/unnecessary-return` — `return;` as the last thing a void method does, `continue;` as
//! the last thing a loop body does.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["clutter", "redundant"],
    remediation_minutes: 1,
    description: r#"Detects a `return;` that is the last statement executed by a `void` method,
a constructor, or a block lambda — directly at the end of the body, or at the end of an
`if`/`else` branch, a `try` block, or a `catch` block that itself ends the body — and, in the
same way, a `continue;` that is the last statement executed by a loop body.

**Why it matters.** Control leaves the method (or goes on with the next iteration) there
anyway. The extra statement is noise,
and it can make readers look for code after it that they might be skipping.

**How to fix.** Remove the `return;` (or `continue;`).

```java
// Noncompliant
void reset() {
    counter = 0;
    return;
}

for (Item item : items) {
    if (item.isStale()) {
        evict(item);
        continue;
    }
}

// Compliant
void reset() {
    counter = 0;
}

for (Item item : items) {
    if (item.isStale()) {
        evict(item);
    }
}
```

A `return;` that skips later statements, or that ends a loop body or a `switch` case, is
not reported; neither is a `continue` that skips later statements, ends a `switch` case
(removing it would fall through), or names a label."#,
    ..RuleMeta::new("java/unnecessary-return", "java", "Unnecessary return")
};

pub struct UnnecessaryReturn;

/// What a jump statement would end anyway.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    /// `return;`: the enclosing function.
    Function,
    /// `continue;`: the body of the enclosing loop.
    LoopBody,
}

fn is_loop(k: &str) -> bool {
    matches!(
        k,
        "for_statement" | "enhanced_for_statement" | "while_statement" | "do_statement"
    )
}

/// Whether `stmt` is the last thing executed by its function (or loop body) when it completes.
fn ends_function<'t>(ctx: &Ctx<'t>, stmt: Node<'t>, target: Target) -> bool {
    let mut cur = stmt;
    for _ in 0..64 {
        let Some(parent) = ctx.parent(cur) else {
            return false;
        };
        match kind(parent) {
            "block" | "constructor_body" => {
                if util::last_named(parent).is_none_or(|l| l.id() != cur.id()) {
                    return false;
                }
                let Some(gp) = ctx.parent(parent) else {
                    return false;
                };
                match kind(gp) {
                    "method_declaration" | "constructor_declaration" | "lambda_expression" => {
                        return target == Target::Function;
                    }
                    k if is_loop(k) => {
                        return target == Target::LoopBody
                            && util::fld(gp, F.body).is_some_and(|b| b.id() == parent.id());
                    }
                    // A nested block, an `if` branch, a synchronized or try body.
                    "block"
                    | "if_statement"
                    | "synchronized_statement"
                    | "try_statement"
                    | "try_with_resources_statement" => {
                        cur = if kind(gp) == "block" { parent } else { gp };
                    }
                    "catch_clause" => match ctx.parent(gp) {
                        Some(t)
                            if matches!(
                                kind(t),
                                "try_statement" | "try_with_resources_statement"
                            ) =>
                        {
                            cur = t
                        }
                        _ => return false,
                    },
                    _ => return false,
                }
            }
            // `for (…) continue;` without braces.
            k if is_loop(k) => {
                return target == Target::LoopBody
                    && util::fld(parent, F.body).is_some_and(|b| b.id() == cur.id());
            }
            // `if (c) return;` without braces, `else if` chains.
            "if_statement" => {
                let is_branch = util::fld(parent, F.consequence)
                    .is_some_and(|c| c.id() == cur.id())
                    || util::fld(parent, F.alternative).is_some_and(|a| a.id() == cur.id());
                if !is_branch {
                    return false;
                }
                cur = parent;
            }
            _ => return false,
        }
    }
    false
}

impl<'t> JavaRule<'t> for UnnecessaryReturn {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["return_statement", "continue_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        // `return value;` / `continue label;`
        if util::first_named(node).is_some() {
            return;
        }
        if kind(node) == "continue_statement" {
            if ends_function(ctx, node, Target::LoopBody) {
                ctx.emit(node, "Remove this redundant `continue`");
            }
        } else if ends_function(ctx, node, Target::Function) {
            ctx.emit(node, "Remove this redundant `return`");
        }
    }
}
