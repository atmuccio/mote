//! `java/index-of-positive-check` — `s.indexOf(x) > 0` ignores a match at index 0.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["strings", "collections"],
    remediation_minutes: 2,
    description: r#"Detects the result of `String.indexOf(..)` or `List.indexOf(..)` compared with
`> 0` (or `0 <`).

**Why it matters.** `indexOf` returns `-1` when nothing is found and `0` when the match is
at the very beginning. `> 0` therefore treats a match at index 0 as "not found": a string
that *starts* with the separator or a list whose *first* element matches takes the wrong
branch. The intended test is almost always `>= 0` (or `contains(..)`).

**How to fix.** Use `>= 0` / `!= -1`, or `contains(..)`. If a match at position 0 really
must be excluded, make that explicit (`idx > 0` on a named variable, with a comment).

```java
// Noncompliant
if (path.indexOf('/') > 0) { … }

// Compliant
if (path.indexOf('/') >= 0) { … }
if (path.contains("/")) { … }
```

Only direct comparisons of an `indexOf(..)` call on a receiver proven to be a `String`,
`StringBuilder`, or `List` are reported, and not when the guarded code works with positions
(`substring`, `indexOf`, `charAt`) — there `> 0` usually deliberately requires a non-empty
prefix, as in `if (s.indexOf(':') > 0) key = s.substring(0, s.indexOf(':'));`."#,
    ..RuleMeta::new(
        "java/index-of-positive-check",
        "java",
        "indexOf() result checked with > 0",
    )
};

#[derive(Default)]
pub struct IndexOfPositiveCheck {
    index_of: util::Gate,
}

/// Whether the code guarded by the comparison works with positions (`substring`, `indexOf`,
/// `charAt`): then `> 0` usually means "found, with a non-empty prefix" on purpose.
fn position_is_used<'t>(ctx: &Ctx<'t>, cmp: Node<'t>) -> bool {
    let uses_position = |n: Node<'t>| {
        let t = ctx.text(n);
        t.contains("substring(") || t.contains("indexOf(") || t.contains("charAt(")
    };
    let mut prev = cmp;
    for a in ctx.ancestors(cmp) {
        match kind(a) {
            "parenthesized_expression" | "unary_expression" => {}
            "binary_expression" => {
                if util::op_str(a) == "&&"
                    && util::fld(a, F::Right)
                        .is_some_and(|r| r.id() != prev.id() && uses_position(r))
                {
                    return true;
                }
            }
            "if_statement" | "ternary_expression" | "while_statement" => {
                let in_condition = util::fld(a, F::Condition).is_some_and(|c| c.id() == prev.id());
                let body = if kind(a) == "while_statement" {
                    F::Body
                } else {
                    F::Consequence
                };
                return in_condition && util::fld(a, body).is_some_and(uses_position);
            }
            _ => return false,
        }
        prev = a;
    }
    false
}

impl<'t> JavaRule<'t> for IndexOfPositiveCheck {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["binary_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let (call, zero) = match ctx.operator_str(node) {
            ">" => (util::fld(node, F::Left), util::fld(node, F::Right)),
            "<" => (util::fld(node, F::Right), util::fld(node, F::Left)),
            _ => return,
        };
        if !self.index_of.open(ctx, "indexOf(") {
            return;
        }
        let (Some(call), Some(zero)) = (call, zero) else {
            return;
        };
        let call = syntax::strip_parens(call);
        if kind(call) != "method_invocation" || !syntax::is_int_literal(zero, ctx.source(), 0) {
            return;
        }
        let Some(name) = util::fld(call, F::Name) else {
            return;
        };
        if ctx.text(name) != "indexOf" {
            return;
        }
        let Some(recv) = util::fld(call, F::Object) else {
            return;
        };
        let known = kind(recv) == "string_literal"
            || ctx.type_of(recv).is_some_and(|t| {
                t.is_string()
                    || t.is_class("java.lang.StringBuilder")
                    || t.is_class("java.lang.StringBuffer")
                    || ctx.is_subtype_of(&t, "java.util.List")
            });
        if !known || position_is_used(ctx, node) {
            return;
        }
        ctx.emit(
            node,
            "Use `>= 0` (or contains()): indexOf() returns 0 for a match at the start, which `> 0` treats as not found",
        );
    }
}
