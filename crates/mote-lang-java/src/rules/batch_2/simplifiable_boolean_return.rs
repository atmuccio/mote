//! `java/simplifiable-boolean-return` — `if (c) return true; else return false;`.

use super::util::{field, keyword, next_statement, single_statement, FIELDS};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["readability", "redundant"],
    remediation_minutes: 2,
    description: r#"Detects an `if` statement that returns `true` in one branch and `false` in the
other — either with an `else`, or with the second `return` directly after the `if`.

**Why it matters.** The condition already is the boolean being returned. Wrapping it in an
`if` turns one line into four and makes readers check both branches to understand a simple
expression.

**How to fix.** Return the condition (negated with `!` when the branches are inverted).

```java
// Noncompliant
if (list.isEmpty()) {
    return true;
} else {
    return false;
}

// Compliant
return list.isEmpty();
```

Not reported: the last `if` of a chain of guards that all return the same value
(`if (a != o.a) return false; if (b != o.b) return false; return true;`, typical of
`equals`), where keeping the parallel structure is clearer. Local variable declarations between
the guards (`Point p = (Point) o;`) do not break the chain, and an `if`/`else if` whose
branches all return that value (or hold nothing but such guards) counts as a guard as well.
After a guard returning the
opposite value (`if (x == null) return false; if (x.isEmpty()) return true; return false;`)
the last `if` is reported: `return x.isEmpty();` says the same in one line."#,
    ..RuleMeta::new(
        "java/simplifiable-boolean-return",
        "java",
        "Boolean return wrapped in an if statement",
    )
};

pub struct SimplifiableBooleanReturn;

/// `Some(true)` / `Some(false)` for `return true;` / `return false;` (possibly as the only
/// statement of a block).
fn returned_literal(stmt: Node<'_>) -> Option<bool> {
    let s = single_statement(stmt)?;
    if kind(s) != "return_statement" {
        return None;
    }
    match syntax::first_named_child(s).map(|e| kind(syntax::strip_parens(e))) {
        Some("true") => Some(true),
        Some("false") => Some(false),
        _ => None,
    }
}

/// The previous sibling statement, skipping comments.
fn previous_statement<'t>(ctx: &Ctx<'t>, node: Node<'t>) -> Option<Node<'t>> {
    let mut cur = ctx.prev_named_sibling(node);
    while let Some(n) = cur {
        if !syntax::is_comment(n) {
            return Some(n);
        }
        cur = ctx.prev_named_sibling(n);
    }
    None
}

/// The statement before `node` that may be a guard: local variable declarations in between
/// (`Point p = (Point) o;` in an `equals`) are skipped.
fn previous_guard<'t>(ctx: &Ctx<'t>, node: Node<'t>) -> Option<Node<'t>> {
    let mut cur = previous_statement(ctx, node);
    for _ in 0..8 {
        match cur {
            Some(s) if kind(s) == "local_variable_declaration" => cur = previous_statement(ctx, s),
            _ => break,
        }
    }
    cur
}

/// Whether `stmt` is a guard returning `lit`: `if (…) return lit;` without `else`, or an
/// `if`/`else if` chain whose every branch returns `lit` or holds only such a guard
/// (`if (a == null) { if (o.a != null) return false; } else if (!a.equals(o.a)) return false;`).
fn is_guard(stmt: Node<'_>, lit: bool, depth: u32) -> bool {
    if kind(stmt) != "if_statement" || depth > 4 {
        return false;
    }
    let branch = |b: Node<'_>| {
        returned_literal(b) == Some(lit)
            || single_statement(b).is_some_and(|s| is_guard(s, lit, depth + 1))
    };
    field(stmt, FIELDS.consequence).is_some_and(branch)
        && field(stmt, FIELDS.alternative).is_none_or(branch)
}

impl<'t> JavaRule<'t> for SimplifiableBooleanReturn {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["if_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(cons) = field(node, FIELDS.consequence) else {
            return;
        };
        let Some(first) = returned_literal(cons) else {
            return;
        };
        let second = match field(node, FIELDS.alternative) {
            Some(alt) => returned_literal(alt),
            None => {
                let in_block = ctx.parent(node).is_some_and(|p| {
                    matches!(
                        kind(p),
                        "block" | "constructor_body" | "switch_block_statement_group"
                    )
                });
                // The last guard of a chain returning the same value (`if (a != o.a) return
                // false; if (b != o.b) return false; return true;`, as in `equals`) reads best
                // like its siblings.
                if !in_block || previous_guard(ctx, node).is_some_and(|g| is_guard(g, first, 0)) {
                    return;
                }
                next_statement(ctx, node).and_then(|s| {
                    (kind(s) == "return_statement")
                        .then(|| returned_literal(s))
                        .flatten()
                })
            }
        };
        if second != Some(!first) {
            return;
        }
        let msg = if first {
            "Return the condition directly instead of returning true and false from an if"
        } else {
            "Return the negated condition directly instead of returning false and true from an if"
        };
        ctx.emit(keyword(node), msg);
    }
}
