//! `java/identical-operands` — the same expression on both sides of a binary operator.

use super::util::{self, Fld, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["suspicious", "typo"],
    remediation_minutes: 5,
    description: r#"Detects binary operations whose two operands are the same side-effect-free
expression — `a == a`, `x < x`, `flag && flag`, `n - n`, `mask | mask` — and `a.equals(a)`.

**Why it matters.** The result is fixed (`true`, `false`, `0`, `1`, or the operand itself), so
the expression doesn't test or compute what it appears to. It is nearly always a copy-paste
slip where one side should name a different variable (`a.x == b.x` written as `a.x == a.x`).

**How to fix.** Correct the operand that was meant to differ, or replace the expression with
its constant value if that really is the intent.

```java
// Noncompliant
if (left.getId() == left.getId() && width == width) { ... }
return this.start - this.start;

// Compliant
if (left.getId() == right.getId() && width == other.width) { ... }
return this.start - other.start;
```

Not reported: operands containing calls, `new`, assignments, or increments (they may yield
different values); two literals; `x == x` / `x != x` and other comparisons or arithmetic on
`float`/`double` values (the NaN-test idiom) or on values whose type is unknown; and
`a.equals(a)` in test code, where it checks that `equals` is reflexive."#,
    ..RuleMeta::new(
        "java/identical-operands",
        "java",
        "Identical expressions on both sides of an operator",
    )
};

pub struct IdenticalOperands;

impl IdenticalOperands {
    fn check_binary<'t>(node: Node<'t>, ctx: &Ctx<'t>) {
        let op = ctx.operator_str(node);
        let needs_type = match op {
            "==" | "!=" | "<" | ">" | "<=" | ">=" | "-" | "/" | "%" => true,
            "&&" | "||" | "^" | "&" | "|" => false,
            _ => return,
        };
        let (Some(left), Some(right)) = (node.fld(F::Left), node.fld(F::Right)) else {
            return;
        };
        if !util::same_code(left, right, ctx.source()) {
            return;
        }
        if util::is_literal(left) || util::has_side_effects(left) {
            return;
        }
        if needs_type {
            // Unknown or floating-point operands may be a deliberate NaN test.
            let Some(t) = ctx.type_of(left) else { return };
            if util::is_floating(&t) || !(t.is_primitive() || t.is_reference()) {
                return;
            }
        }
        let at = util::op(node).unwrap_or(node);
        ctx.emit(
            at,
            format!(
                "Correct one of the identical operands of `{op}`; the result doesn't depend on them"
            ),
        );
    }

    fn check_equals<'t>(node: Node<'t>, ctx: &Ctx<'t>) {
        if ctx.is_test() || !ctx.text(node).contains("equals") {
            return;
        }
        if util::method_name(ctx, node) != "equals" {
            return;
        }
        let Some(recv) = util::plain_receiver(node) else {
            return;
        };
        let mut args = util::args(node);
        let (Some(arg), None) = (args.next(), args.next()) else {
            return;
        };
        if !util::same_code(recv, arg, ctx.source()) || util::has_side_effects(recv) {
            return;
        }
        if util::is_literal(recv) {
            return;
        }
        let at = node.fld(F::Name).unwrap_or(node);
        ctx.emit(
            at,
            "Compare with the intended object; an object always equals itself",
        );
    }
}

impl<'t> JavaRule<'t> for IdenticalOperands {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["binary_expression", "method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["equals"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if kind(node) == "binary_expression" {
            Self::check_binary(node, ctx);
        } else {
            Self::check_equals(node, ctx);
        }
    }
}
