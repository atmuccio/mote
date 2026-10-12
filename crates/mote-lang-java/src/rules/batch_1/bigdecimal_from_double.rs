//! `java/bigdecimal-from-double` — `new BigDecimal(0.1)`.

use super::util::{self, Fld, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["numbers", "precision"],
    remediation_minutes: 5,
    description: r#"Detects `new BigDecimal(x)` (and `new BigDecimal(x, mathContext)`) where `x`
is a floating-point literal or an expression of type `double`/`float`.

**Why it matters.** Most decimal fractions have no exact binary representation, and this
constructor preserves the binary value exactly: `new BigDecimal(0.1)` is
`0.1000000000000000055511151231257827021181583404541015625`. The error then leaks into
amounts, comparisons (`compareTo`, `equals`), and rounding — the very things `BigDecimal` is
used to get right.

**How to fix.** Build the value from its decimal text: `new BigDecimal("0.1")`, or from a
`double` with `BigDecimal.valueOf(d)`, which goes through `Double.toString` and yields `0.1`.

```java
// Noncompliant
BigDecimal rate = new BigDecimal(0.07);
BigDecimal total = new BigDecimal(price * quantity);

// Compliant
BigDecimal rate = new BigDecimal("0.07");
BigDecimal total = BigDecimal.valueOf(price * quantity);
```

Literals whose value is exactly representable (`new BigDecimal(1.0)`, `new BigDecimal(0.5)`)
are not reported, and neither are arguments whose type is unknown or a `BigDecimal` converted
to `float`/`double` on the spot (`new BigDecimal(amount.floatValue())`): the code already holds
the exact decimal value and asks on purpose what the binary type makes of it."#,
    ..RuleMeta::new(
        "java/bigdecimal-from-double",
        "java",
        "BigDecimal constructed from a double",
    )
};

pub struct BigDecimalFromDouble;

/// `bd.floatValue()` / `bd.doubleValue()` with `bd` a `BigDecimal`: the code already has the
/// exact decimal value and goes through `float`/`double` only to see what the binary type makes
/// of it (typically a test comparing against the exact binary expansion).
fn is_binary_round_trip<'t>(ctx: &Ctx<'t>, arg: Node<'t>) -> bool {
    if kind(arg) != "method_invocation" || syntax::argument_count(arg) != 0 {
        return false;
    }
    if !matches!(ctx.call_name(arg), Some("floatValue" | "doubleValue")) {
        return false;
    }
    arg.fld(F::Object)
        .and_then(|o| ctx.type_of(o))
        .is_some_and(|t| t.is_class("java.math.BigDecimal"))
}

/// Whether a decimal floating-point literal denotes a value that `double` (or `float`, with an
/// `f` suffix) represents exactly. Unparseable or huge literals count as inexact.
pub(crate) fn literal_is_exact(text: &str) -> bool {
    let t: String = text.chars().filter(|c| *c != '_').collect();
    let lower = t.to_ascii_lowercase();
    if lower.starts_with("0x") {
        // Hexadecimal floating-point literals are binary by construction.
        return true;
    }
    let is_float = lower.ends_with('f');
    let body = lower.trim_end_matches(['f', 'd']);
    let (mantissa, exp) = match body.split_once('e') {
        Some((m, e)) => match e.parse::<i32>() {
            Ok(v) => (m, v),
            Err(_) => return false,
        },
        None => (body, 0),
    };
    let (int_part, frac_part) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits = format!("{int_part}{frac_part}");
    let digits = digits.trim_start_matches('0');
    if digits.is_empty() {
        return true; // zero
    }
    if digits.len() > 38 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let Ok(mut d) = digits.parse::<u128>() else {
        return false;
    };
    // value = d * 10^(-scale)
    let mut scale = frac_part.len() as i64 - exp as i64;
    while d % 10 == 0 {
        d /= 10;
        scale -= 1;
    }
    let mantissa_bits = if is_float { 24 } else { 53 };
    let odd_part = |mut v: u128| {
        while v % 2 == 0 {
            v /= 2;
        }
        v
    };
    let fits = |v: u128| odd_part(v) < (1u128 << mantissa_bits);
    if scale <= 0 {
        // d * 10^k = d * 5^k * 2^k
        if -scale > 55 {
            return false;
        }
        let Some(p) = 5u128.checked_pow((-scale) as u32) else {
            return false;
        };
        match d.checked_mul(p) {
            Some(v) => fits(v),
            None => false,
        }
    } else {
        // d / (2^s * 5^s): needs 5^s | d.
        if scale > 55 {
            return false;
        }
        let Some(p) = 5u128.checked_pow(scale as u32) else {
            return false;
        };
        d % p == 0 && fits(d / p)
    }
}

impl<'t> JavaRule<'t> for BigDecimalFromDouble {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["object_creation_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(ty) = node.fld(F::Type) else {
            return;
        };
        let text = ctx.text(ty);
        if text.rsplit('.').next() != Some("BigDecimal") {
            return;
        }
        let mut args = util::args(node);
        let Some(arg) = args.next() else { return };
        if args.nth(1).is_some() {
            return; // (double, MathContext) has two arguments; more is another constructor
        }
        let inner = syntax::strip_parens(arg);
        let is_double = if kind(inner) == "decimal_floating_point_literal" {
            !literal_is_exact(ctx.text(inner))
        } else if kind(inner) == "unary_expression"
            && matches!(util::op_str(inner), "-" | "+")
            && inner
                .fld(F::Operand)
                .is_some_and(|o| kind(syntax::strip_parens(o)) == "decimal_floating_point_literal")
        {
            let lit = inner
                .fld(F::Operand)
                .map(syntax::strip_parens)
                .map(|o| ctx.text(o))
                .unwrap_or("");
            !literal_is_exact(lit)
        } else if util::is_literal(inner) || is_binary_round_trip(ctx, inner) {
            false
        } else {
            ctx.type_of(arg).is_some_and(|t| util::is_floating(&t))
        };
        if !is_double {
            return;
        }
        if !ctx
            .resolve_type(ty)
            .is_some_and(|t| t.is_class("java.math.BigDecimal"))
        {
            return;
        }
        ctx.emit(
            arg,
            "Use BigDecimal.valueOf(..) or new BigDecimal(\"..\"); this constructor keeps the binary rounding error of the double",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::literal_is_exact;

    #[test]
    fn exact_literals() {
        for t in [
            "1.0", "0.5", "0.25", "100d", "1e3", "0.0", "2.5e-1", "1_000.0", "0x1.8p1", "0.125f",
            "3.0F",
        ] {
            assert!(literal_is_exact(t), "{t}");
        }
        for t in [
            "0.1",
            "0.07",
            "1.1",
            "3.14159",
            "1e-3",
            "0.1f",
            "123456789012345678901.5",
            "9007199254740993.0",
        ] {
            assert!(!literal_is_exact(t), "{t}");
        }
    }
}
