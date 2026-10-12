//! `java/float-equality` — `==` / `!=` on floating-point values.

use super::util::{self, Fld, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["numbers", "precision"],
    remediation_minutes: 5,
    description: r#"Detects `==` and `!=` comparisons where an operand is a `float` or `double`
value.

**Why it matters.** Floating-point arithmetic rounds at almost every step, so two
computations that are mathematically equal often differ in the last bits: `0.1 + 0.2 == 0.3`
is `false`. Exact comparisons therefore fail intermittently, depending on the inputs.

**How to fix.** Compare with a tolerance suited to the magnitudes involved
(`Math.abs(a - b) < EPSILON`), use `BigDecimal` for decimal quantities such as money, or
`Double.compare(a, b) == 0` when bitwise-exact equality really is wanted.

```java
// Noncompliant
if (total == expected) { ... }

// Compliant
if (Math.abs(total - expected) < 1e-9) { ... }
```

Not reported: `x != x` / `x == x` (the NaN test), comparisons with an integral constant such
as `0`, `-1`, or `1.0` (sentinels and exact guards like `if (divisor == 0.0)`) or with a
`static final` constant of this file holding one (`AUTO_DPI = -1f`), the two ends of a range
compared right before dividing by their difference (`if (max == min) return lo;` guarding
`/ (max - min)`, an exact test for a zero divisor), comparisons
with `MAX_VALUE`, `MIN_VALUE`, or the infinities, whole-number tests against the rounded value
(`x == Math.floor(x)`, `x == (long) x`), and operands whose type is unknown."#,
    ..RuleMeta::new(
        "java/float-equality",
        "java",
        "Floating-point values compared with == or !=",
    )
};

pub struct FloatEquality;

/// Integral-valued numeric literal (`0`, `-1`, `1.0`, `2d`, `0x10`).
fn is_integral_literal(ctx: &Ctx<'_>, node: Node<'_>) -> bool {
    let mut n = syntax::strip_parens(node);
    while kind(n) == "unary_expression" && matches!(util::op_str(n), "-" | "+") {
        match n.fld(F::Operand) {
            Some(o) => n = syntax::strip_parens(o),
            None => return false,
        }
    }
    match kind(n) {
        "decimal_integer_literal"
        | "hex_integer_literal"
        | "octal_integer_literal"
        | "binary_integer_literal"
        | "character_literal" => true,
        "decimal_floating_point_literal" => {
            let t = ctx.text(n).trim_end_matches(['d', 'D', 'f', 'F']);
            if t.contains(['e', 'E']) {
                return false;
            }
            match t.split_once('.') {
                Some((_, frac)) => frac.bytes().all(|b| b == b'0' || b == b'_'),
                None => true,
            }
        }
        _ => false,
    }
}

/// Exact sentinel constants (`Double.MAX_VALUE`, `Float.NEGATIVE_INFINITY`, …).
fn is_sentinel_constant(ctx: &Ctx<'_>, node: Node<'_>) -> bool {
    let n = syntax::strip_parens(node);
    let name = match kind(n) {
        "field_access" => n.fld(F::Field).map(|f| ctx.text(f)),
        "identifier" => Some(ctx.text(n)),
        _ => None,
    };
    matches!(
        name,
        Some("MAX_VALUE" | "MIN_VALUE" | "POSITIVE_INFINITY" | "NEGATIVE_INFINITY" | "MIN_NORMAL")
    )
}

/// `Math.floor(x)`, `Math.ceil(x)`, `Math.rint(x)`, or a cast to an integral type: comparing a
/// value with its rounded self is the exact "is this a whole number?" test.
fn is_rounding(ctx: &Ctx<'_>, node: Node<'_>) -> bool {
    let n = syntax::strip_parens(node);
    match kind(n) {
        "method_invocation" => {
            matches!(
                n.fld(F::Name).map(|m| ctx.text(m)),
                Some("floor" | "ceil" | "rint" | "round")
            ) && n
                .fld(F::Object)
                .is_some_and(|o| matches!(ctx.text(o), "Math" | "StrictMath" | "java.lang.Math"))
        }
        "cast_expression" => n.fld(F::Type).is_some_and(|t| kind(t) == "integral_type"),
        _ => false,
    }
}

/// A `static final` field (or interface constant) of this file whose value is an integral
/// literal: a named sentinel such as `static final float AUTO = -1f`.
fn is_integral_constant<'t>(ctx: &Ctx<'t>, node: Node<'t>) -> bool {
    let n = syntax::strip_parens(node);
    if kind(n) != "identifier" {
        return false;
    }
    let Some(d) = ctx.declaration_of(n) else {
        return false;
    };
    d.kind == VarKind::Field
        && (kind(d.statement) == "constant_declaration"
            || (syntax::has_modifier(d.statement, "static")
                && syntax::has_modifier(d.statement, "final")))
        && d.value.is_some_and(|v| is_integral_literal(ctx, v))
}

/// `a == b` where the same function divides by `(a - b)` or `(b - a)`: the comparison is the
/// exact test for a zero divisor (`if (max == min) return lo; … / (max - min)`).
fn guards_division<'t>(ctx: &Ctx<'t>, node: Node<'t>, left: Node<'t>, right: Node<'t>) -> bool {
    let simple = |n: Node<'_>| matches!(kind(n), "identifier" | "field_access");
    let (l, r) = (syntax::strip_parens(left), syntax::strip_parens(right));
    if !simple(l) || !simple(r) {
        return false;
    }
    let Some(func) = ctx.enclosing_function(node) else {
        return false;
    };
    let squeeze = |t: &str| t.chars().filter(|c| !c.is_whitespace()).collect::<String>();
    let (a, b) = (squeeze(ctx.text(l)), squeeze(ctx.text(r)));
    let body = squeeze(ctx.text(func));
    body.contains(&format!("/({a}-{b})")) || body.contains(&format!("/({b}-{a})"))
}

impl<'t> JavaRule<'t> for FloatEquality {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["binary_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let op = ctx.operator_str(node);
        if op != "==" && op != "!=" {
            return;
        }
        let (Some(left), Some(right)) = (node.fld(F::Left), node.fld(F::Right)) else {
            return;
        };
        if syntax::is_null_literal(left) || syntax::is_null_literal(right) {
            return;
        }
        if is_integral_literal(ctx, left) || is_integral_literal(ctx, right) {
            return;
        }
        if is_sentinel_constant(ctx, left) || is_sentinel_constant(ctx, right) {
            return;
        }
        if is_rounding(ctx, left) || is_rounding(ctx, right) {
            return;
        }
        if is_integral_constant(ctx, left) || is_integral_constant(ctx, right) {
            return;
        }
        if util::same_code(left, right, ctx.source()) {
            return; // NaN test
        }
        let Some(lt) = ctx.type_of(left) else { return };
        if !lt.is_numeric() {
            return; // objects, enums, booleans: no need to type the other side
        }
        let Some(rt) = ctx.type_of(right) else { return };
        // Boxed == boxed is a reference comparison (another problem); a primitive on either side
        // makes it a numeric comparison.
        let numeric =
            (lt.is_primitive() || rt.is_primitive()) && lt.is_numeric() && rt.is_numeric();
        if !numeric {
            return;
        }
        if !(util::is_floating(&lt) || util::is_floating(&rt)) {
            return;
        }
        if guards_division(ctx, node, left, right) {
            return;
        }
        let at = util::op(node).unwrap_or(node);
        ctx.emit(
            at,
            format!("Avoid `{op}` on floating-point values; compare with a tolerance or Double.compare()"),
        );
    }
}
