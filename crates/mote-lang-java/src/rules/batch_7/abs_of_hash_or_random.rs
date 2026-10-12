//! `java/abs-of-hash-or-random` — `Math.abs(x.hashCode())` can still be negative.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["numbers", "overflow"],
    remediation_minutes: 5,
    description: r#"Detects `Math.abs(..)` applied directly to a hash code (`x.hashCode()`,
`Objects.hash(..)`, `Objects.hashCode(..)`, `Arrays.hashCode(..)`) or to an unbounded random
integer (`random.nextInt()`, `random.nextLong()`), and the same done by hand: `h < 0 ? -h : h` or
`if (h < 0) h = -h;` on such a call or on a local variable initialized with one.

**Why it matters.** These values range over every `int` (or `long`), including
`Integer.MIN_VALUE`, whose absolute value does not fit: `Math.abs(Integer.MIN_VALUE)` is
`Integer.MIN_VALUE`, still negative. Code that uses the result as an array index, a bucket or
a shard number (`Math.abs(key.hashCode()) % n`) then fails with a negative index — rarely, for
one unlucky key, which makes the bug hard to reproduce.

**How to fix.** Reduce first, or clear the sign bit.

```java
// Noncompliant
int bucket = Math.abs(key.hashCode()) % buckets;

// Compliant
int bucket = Math.floorMod(key.hashCode(), buckets);
int other = (key.hashCode() & 0x7fffffff) % buckets;
```

`Math.abs(h % n)` (already reduced) and bounded random calls (`nextInt(bound)`) are not
reported. Random receivers must provably be a `java.util.Random`."#,
    ..RuleMeta::new(
        "java/abs-of-hash-or-random",
        "java",
        "Math.abs of a hash code or random integer",
    )
};

pub struct AbsOfHashOrRandom;

/// What `inner` is when it is a hash code or an unbounded random number.
fn source_kind<'t>(inner: Node<'t>, ctx: &Ctx<'t>) -> Option<&'static str> {
    let name = ctx.call_name(inner)?;
    match name {
        "hashCode" if syntax::argument_count(inner) == 0 => Some("a hash code"),
        "hashCode" | "hash" => {
            let o = inner.field(Field::Object)?;
            (util::names_type(ctx, o, "java.util.Objects")
                || (name == "hashCode" && util::names_type(ctx, o, "java.util.Arrays")))
            .then_some("a hash code")
        }
        "nextInt" | "nextLong" if syntax::argument_count(inner) == 0 => {
            util::receiver_is(ctx, inner, &["java.util.Random"])
                .then_some("an unbounded random number")
        }
        _ => None,
    }
}

/// For `-v`: what `v` is when it is such a call, or a local variable initialized with one.
fn negated_source<'t>(operand: Node<'t>, neg: Node<'t>, ctx: &Ctx<'t>) -> Option<&'static str> {
    let operand = syntax::strip_parens(operand);
    match kind(operand) {
        "method_invocation" => source_kind(operand, ctx),
        "identifier" => {
            let decl = ctx.declaration_of(operand)?;
            if decl.kind != VarKind::Local {
                return None;
            }
            let what = source_kind(syntax::strip_parens(decl.value?), ctx)?;
            // The variable must still hold the hash: any other write (`h = h % n`, `h++`)
            // may have made it safe. The `h = -h` being checked is not counted.
            let name = ctx.text(operand);
            let func = ctx.enclosing_function(neg)?;
            let (ns, ne) = (neg.start_byte(), neg.end_byte());
            let rewritten = util::any_in_scope(func, |n| match kind(n) {
                "assignment_expression" => {
                    n.field(Field::Left)
                        .is_some_and(|l| ctx.text(syntax::strip_parens(l)) == name)
                        && !(n.start_byte() <= ns && ne <= n.end_byte())
                }
                "update_expression" => syntax::first_named_child(n)
                    .is_some_and(|o| ctx.text(syntax::strip_parens(o)) == name),
                _ => false,
            });
            (!rewritten).then_some(what)
        }
        _ => None,
    }
}

/// `(v < 0) ? -v : v`, `(v >= 0) ? v : -v`, and `if (v < 0) v = -v;`: a hand-written `abs`.
fn manual_abs<'t>(neg: Node<'t>, operand: Node<'t>, ctx: &Ctx<'t>) -> bool {
    let target = ctx.text(syntax::strip_parens(operand));
    // The comparison `v < 0` (or `0 > v`) and whether it tests "negative".
    let tests_sign = |cond: Node<'t>| -> Option<bool> {
        let cond = syntax::strip_parens(cond);
        if kind(cond) != "binary_expression" {
            return None;
        }
        let (l, r) = (cond.field(Field::Left)?, cond.field(Field::Right)?);
        let zero = |n: Node<'t>| ctx.text(n) == "0" || ctx.text(n) == "0L";
        let op = syntax::operator_str(cond);
        if ctx.text(syntax::strip_parens(l)) == target && zero(r) {
            match op {
                "<" | "<=" => Some(true),
                ">" | ">=" => Some(false),
                _ => None,
            }
        } else if zero(l) && ctx.text(syntax::strip_parens(r)) == target {
            match op {
                ">" | ">=" => Some(true),
                "<" | "<=" => Some(false),
                _ => None,
            }
        } else {
            None
        }
    };
    let mut child = neg;
    for a in ctx.ancestors(neg) {
        match kind(a) {
            "parenthesized_expression" => {
                child = a;
                continue;
            }
            "ternary_expression" => {
                let Some(negative) = a.field(Field::Condition).and_then(tests_sign) else {
                    return false;
                };
                let slot = if negative {
                    Field::Consequence
                } else {
                    Field::Alternative
                };
                return a.field(slot).is_some_and(|s| s.id() == child.id());
            }
            "assignment_expression" => {
                // `v = -v;` directly guarded by `if (v < 0)`.
                if syntax::operator_str(a) != "="
                    || a.field(Field::Right).is_none_or(|r| r.id() != child.id())
                    || a.field(Field::Left).is_none_or(|l| ctx.text(l) != target)
                {
                    return false;
                }
                let mut anc = ctx.ancestors(a);
                let Some(stmt) = anc.next() else { return false };
                let mut up = anc.next();
                if up.is_some_and(|b| kind(b) == "block") {
                    up = anc.next();
                }
                return kind(stmt) == "expression_statement"
                    && up.is_some_and(|i| {
                        kind(i) == "if_statement"
                            && i.field(Field::Condition).and_then(tests_sign) == Some(true)
                    });
            }
            _ => return false,
        }
    }
    false
}

impl<'t> JavaRule<'t> for AbsOfHashOrRandom {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation", "unary_expression"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["abs"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if kind(node) == "unary_expression" {
            if syntax::operator_str(node) != "-" {
                return;
            }
            let Some(operand) = node.field(Field::Operand) else {
                return;
            };
            if !matches!(
                kind(syntax::strip_parens(operand)),
                "identifier" | "method_invocation"
            ) {
                return;
            }
            if !manual_abs(node, operand, ctx) {
                return;
            }
            let Some(what) = negated_source(operand, node, ctx) else {
                return;
            };
            ctx.emit(
                node,
                format!("Use `Math.floorMod` or mask the sign bit: negating {what} leaves MIN_VALUE negative"),
            );
            return;
        }
        if ctx.call_name(node) != Some("abs") {
            return;
        }
        let mut args = util::args(node);
        let (Some(arg), None) = (args.next(), args.next()) else {
            return;
        };
        let inner = syntax::strip_parens(arg);
        let Some(what) = source_kind(inner, ctx) else {
            return;
        };
        let Some(recv) = node.field(Field::Object) else {
            return;
        };
        if !util::names_math(ctx, recv) {
            return;
        }
        ctx.emit(
            node,
            format!("Use `Math.floorMod` or mask the sign bit: `Math.abs` of {what} is negative for MIN_VALUE"),
        );
    }
}
