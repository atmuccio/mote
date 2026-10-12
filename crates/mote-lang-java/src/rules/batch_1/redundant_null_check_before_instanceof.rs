//! `java/redundant-null-check-before-instanceof` — `x != null && x instanceof T`.

use super::util::{self, Fld, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["clutter", "redundant"],
    remediation_minutes: 1,
    description: r#"Detects a `null` check followed by an `instanceof` test of the same
expression: `x != null && x instanceof T`, and its negated form `x == null || !(x instanceof
T)`.

**Why it matters.** `instanceof` is already `false` for `null` (so its negation is `true`),
so the null check adds nothing but length and suggests the author expected otherwise.

**How to fix.** Drop the null check.

```java
// Noncompliant
if (value != null && value instanceof String s) { use(s); }

// Compliant
if (value instanceof String s) { use(s); }
```

The check is also recognised in a longer chain (`a && x != null && x instanceof T`), also
when other terms stand between the two tests, as long as they have no side effects and don't
mention the tested expression (dropping the null check must not change what they do). The
tested expression may be a variable, a field, an array element, or a chain of argument-less
getters (`e.getCause() != null && e.getCause() instanceof IOException`); other method calls
may return a different value the second time and are not compared."#,
    ..RuleMeta::new(
        "java/redundant-null-check-before-instanceof",
        "java",
        "Redundant null check before instanceof",
    )
};

pub struct RedundantNullCheckBeforeInstanceof;

/// `x` of `x <op> null` / `null <op> x` (`op` is `!=` or `==`).
fn null_subject<'t>(n: Node<'t>, op: &str) -> Option<Node<'t>> {
    let n = syntax::strip_parens(n);
    if kind(n) != "binary_expression" || util::op_str(n) != op {
        return None;
    }
    let (l, r) = (n.fld(F::Left)?, n.fld(F::Right)?);
    if syntax::is_null_literal(r) && !syntax::is_null_literal(l) {
        Some(l)
    } else if syntax::is_null_literal(l) && !syntax::is_null_literal(r) {
        Some(r)
    } else {
        None
    }
}

/// Whether evaluating the expression twice yields the same value: names, field accesses,
/// `this`, and calls of argument-less getters (`getCause()`, `isEmpty()` — by the JavaBeans
/// naming convention) on such receivers.
fn is_stable(n: Node<'_>, src: &str, depth: u32) -> bool {
    if depth > 16 {
        return false;
    }
    let n = syntax::strip_parens(n);
    match kind(n) {
        "identifier" | "this" | "super" => true,
        "array_access" => {
            n.fld(F::Array)
                .is_some_and(|a| is_stable(a, src, depth + 1))
                && n.field(Field::Index)
                    .is_some_and(|i| util::is_literal(i) || is_stable(i, src, depth + 1))
        }
        "field_access" => n
            .fld(F::Object)
            .is_some_and(|o| is_stable(o, src, depth + 1)),
        "method_invocation" => {
            let getter = n.fld(F::Name).is_some_and(|m| {
                let name = syntax::node_text(m, src);
                ["get", "is"].iter().any(|p| {
                    name.strip_prefix(p)
                        .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_uppercase()))
                })
            });
            getter
                && util::arg_count(n) == 0
                && n.field(Field::TypeArguments).is_none()
                && n.fld(F::Object)
                    .is_none_or(|o| is_stable(o, src, depth + 1))
        }
        _ => false,
    }
}

/// The `instanceof` test on the right of `&&`, or the negated one on the right of `||`.
fn tested_instanceof(right: Node<'_>, negated: bool) -> Option<Node<'_>> {
    let right = syntax::strip_parens(right);
    let inst = if negated {
        if kind(right) != "unary_expression" || util::op_str(right) != "!" {
            return None;
        }
        syntax::strip_parens(right.fld(F::Operand)?)
    } else {
        right
    };
    (kind(inst) == "instanceof_expression").then_some(inst)
}

impl<'t> JavaRule<'t> for RedundantNullCheckBeforeInstanceof {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["binary_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let op = ctx.operator_str(node);
        let (negated, null_op) = match op {
            "&&" => (false, "!="),
            "||" => (true, "=="),
            _ => return,
        };
        let (Some(left), Some(right)) = (node.fld(F::Left), node.fld(F::Right)) else {
            return;
        };
        let Some(inst) = tested_instanceof(right, negated) else {
            return;
        };
        let Some(tested) = inst.fld(F::Left) else {
            return;
        };
        let src = ctx.source();
        if !is_stable(tested, src, 0) {
            return;
        }
        // Names the tested expression is made of: a term between the null check and the
        // `instanceof` that mentions one of them may rely on the null check.
        let mut tested_names: Vec<&str> = Vec::new();
        let _ = util::any_in_subtree(tested, |n| {
            if kind(n) == "identifier" {
                tested_names.push(syntax::node_text(n, src));
            }
            tested_names.len() > 8
        });
        // Walk the terms of the left-nested `&&` / `||` chain from right to left.
        let mut cur = syntax::strip_parens(left);
        for _ in 0..16 {
            let (term, rest) = if kind(cur) == "binary_expression" && util::op_str(cur) == op {
                match (cur.fld(F::Right), cur.fld(F::Left)) {
                    (Some(r), Some(l)) => (r, Some(l)),
                    _ => return,
                }
            } else {
                (cur, None)
            };
            if null_subject(term, null_op).is_some_and(|s| util::same_code(s, tested, src)) {
                let msg = if negated {
                    "Remove this null check; the negated instanceof is already true for null"
                } else {
                    "Remove this null check; instanceof is already false for null"
                };
                ctx.emit(syntax::strip_parens(term), msg);
                return;
            }
            // A term in between must not change what happens when the null check is dropped:
            // no calls or other effects, and nothing that dereferences the tested value.
            let depends = util::has_side_effects(term)
                || util::any_in_subtree(term, |n| {
                    kind(n) == "identifier" && tested_names.contains(&syntax::node_text(n, src))
                });
            if depends {
                return;
            }
            match rest {
                Some(r) => cur = syntax::strip_parens(r),
                None => return,
            }
        }
    }
}
