//! `js/no-loss-of-precision`

use crate::prelude::*;
use oxc_syntax::number::NumberBase;

pub struct NoLossOfPrecision;

pub static NO_LOSS_OF_PRECISION: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["suspicious"],
    remediation_minutes: 5,
    description: r#"Flags number literals whose written value cannot be stored exactly as a JavaScript number
(a 64-bit float), so the program silently uses a different value.

**Why:** numbers have about 15–17 significant decimal digits and exact integers only up to
2^53. `9007199254740993` becomes `9007199254740992`, `1.0000000000000001` becomes `1`, and `1e400`
becomes `Infinity`. The source then claims a value the program never sees — typically an ID,
a timestamp, or a constant copied from another language.

**Fix:** use a `BigInt` (`9007199254740993n`) or a string for large identifiers, or write the
value with the precision a number can actually hold.

```js
const id = 9007199254740993;     // flagged: stored as 9007199254740992
const id = 9007199254740993n;    // compliant: BigInt
const ratio = 0.1;               // compliant: 0.1 reads back as 0.1
```

A literal is accepted when rounding the stored number to the digits that were written gives back
exactly those digits. A fraction written with 17 or more significant digits — a constant copied
from a table with more precision than any double holds (`0.0640568928626056260850430826`) — is
accepted too, since storing the nearest double (still 15–17 digits) is the expected outcome,
unless rounding visibly changes the value: it collapses to a short number (`1.0000000000000001`
becomes `1`) or changes magnitude (`9.99999999999999999999` becomes `10`). Integers beyond 2^53
are always reported.
"#,
    ..RuleMeta::new(
        "js/no-loss-of-precision",
        "js",
        "Number literals should not lose precision",
    )
};

impl JsRule for NoLossOfPrecision {
    fn meta(&self) -> &'static RuleMeta {
        &NO_LOSS_OF_PRECISION
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::NumericLiteral]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::NumericLiteral(lit) = node.kind() else {
            return;
        };
        let raw = ctx.text(lit.span);
        if raw.is_empty() {
            return;
        }
        let loses = match lit.base {
            NumberBase::Decimal | NumberBase::Float => decimal_loses_precision(raw, lit.value),
            NumberBase::Hex => radix_loses_precision(strip_prefix(raw), 4),
            NumberBase::Binary => radix_loses_precision(strip_prefix(raw), 1),
            NumberBase::Octal => radix_loses_precision(strip_prefix(raw), 3),
        };
        if loses == Some(true) {
            let shown: String = raw.chars().take(40).collect();
            ctx.emit(
                lit.span,
                format!("Rewrite '{shown}': it cannot be represented exactly and would be rounded"),
            );
        }
    }
}

/// Digits after a `0x` / `0o` / `0b` prefix, or after the leading `0` of a legacy octal.
fn strip_prefix(raw: &str) -> &str {
    let bytes = raw.as_bytes();
    if bytes.len() >= 2 && bytes[0] == b'0' && bytes[1].is_ascii_alphabetic() {
        raw.get(2..).unwrap_or("")
    } else {
        raw
    }
}

/// Integer literal written in base 2^`bits_per_digit`: exact iff the span between its highest and
/// lowest set bits fits the 53-bit significand and the value is below 2^1024.
fn radix_loses_precision(digits: &str, bits_per_digit: u32) -> Option<bool> {
    let mut total_bits: u64 = 0;
    let mut first_one: Option<u64> = None;
    let mut last_one: u64 = 0;
    for c in digits.chars() {
        if c == '_' {
            continue;
        }
        let d = c.to_digit(1 << bits_per_digit)?;
        for bit in (0..bits_per_digit).rev() {
            if (d >> bit) & 1 == 1 {
                first_one.get_or_insert(total_bits);
                last_one = total_bits;
            }
            total_bits += 1;
        }
    }
    let Some(first) = first_one else {
        return Some(false); // zero
    };
    let magnitude_bits = total_bits - first; // position of the highest set bit + 1
    let significant_bits = last_one - first + 1;
    Some(significant_bits > 53 || magnitude_bits > 1024)
}

/// Decimal literal (`123`, `1.5e-3`, `.5`, `1_000`): exact enough iff rounding the stored double
/// to the number of significant digits written reproduces those digits.
fn decimal_loses_precision(raw: &str, value: f64) -> Option<bool> {
    let (mantissa, exponent) = match raw.find(['e', 'E']) {
        Some(i) => (raw.get(..i)?, raw.get(i + 1..)?),
        None => (raw, ""),
    };
    let exponent: i64 = if exponent.is_empty() {
        0
    } else {
        let e: String = exponent.chars().filter(|c| *c != '_').collect();
        match e.parse::<i64>() {
            Ok(e) => e,
            // Absurd exponents overflow to Infinity or underflow to 0.
            Err(_) => return Some(mantissa.bytes().any(|b| (b'1'..=b'9').contains(&b))),
        }
    };
    let (int_part, frac_part) = match mantissa.find('.') {
        Some(i) => (mantissa.get(..i)?, mantissa.get(i + 1..)?),
        None => (mantissa, ""),
    };
    let digit = |b: &u8| b.is_ascii_digit();
    let int_digits = || int_part.bytes().filter(digit);
    let frac_digits = || frac_part.bytes().filter(digit);

    // Significant digits: strip leading and trailing zeros of the whole digit sequence.
    let all = || int_digits().chain(frac_digits());
    let leading = all().take_while(|b| *b == b'0').count();
    let total = all().count();
    if leading == total {
        return Some(false); // zero
    }
    let trailing = all().rev().take_while(|b| *b == b'0').count();
    let significant = total - leading - trailing;
    // Decimal exponent of the first significant digit (scientific notation).
    let int_len = int_digits().count() as i64;
    let sci_exponent = int_len - 1 - leading as i64 + exponent;

    if !value.is_finite() || value == 0.0 {
        return Some(true);
    }
    // Up to 15 significant digits always survive a round trip through a normal double.
    if significant <= 15 && value.abs() >= f64::MIN_POSITIVE {
        return Some(false);
    }
    if significant > 800 {
        return Some(true);
    }
    let formatted = format!("{:.*e}", significant - 1, value.abs());
    let (f_mantissa, f_exponent) = formatted.split_once('e')?;
    let f_exponent: i64 = f_exponent.parse().ok()?;
    // A fraction written with 17 or more significant digits (a constant copied from a table,
    // `0.0640568928626056260850430826247450385909`) asks for more precision than any double has;
    // storing the nearest double, which keeps a full 15–17 digits, is the expected outcome. It is
    // still reported when rounding visibly changes the value: a different magnitude
    // (`9.99999999999999999999` becomes `10`) or a collapse to a short number
    // (`1.0000000000000001` becomes `1`).
    let integer_form = exponent >= 0 && frac_digits().all(|b| b == b'0');
    if !integer_form
        && significant >= 17
        && value.abs() >= f64::MIN_POSITIVE
        && f_exponent == sci_exponent
        && shortest_digits(value) >= 15
    {
        return Some(false);
    }
    let written = all().skip(leading).take(significant);
    let stored = f_mantissa.bytes().filter(digit);
    Some(f_exponent != sci_exponent || !written.eq(stored))
}

/// Significant digits of the shortest decimal that reads back as `value`.
fn shortest_digits(value: f64) -> usize {
    let shortest = format!("{:e}", value.abs());
    let mantissa = shortest.split('e').next().unwrap_or("");
    mantissa.bytes().filter(u8::is_ascii_digit).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dec(raw: &str) -> bool {
        let clean: String = raw.chars().filter(|c| *c != '_').collect();
        let value: f64 = clean.parse().unwrap_or(f64::NAN);
        decimal_loses_precision(raw, value) == Some(true)
    }

    #[test]
    fn decimal_literals() {
        assert!(dec("9007199254740993"));
        assert!(!dec("9007199254740992"));
        assert!(!dec("0.1"));
        assert!(!dec("0.30000000000000004"));
        // Over-precise fractions round to the nearest double as expected …
        assert!(!dec("0.0640568928626056260850430826247450385909"));
        assert!(!dec("0.121670472927803391204463153476262425607"));
        // … unless rounding visibly changes the value: a collapse to a short number, or a new
        // magnitude.
        assert!(dec("1.0000000000000001"));
        assert!(dec("5.79999999999999999989"));
        assert!(dec("9.99999999999999999999"));
        // Integers beyond 2^53 are still reported, however they are written.
        assert!(dec("12345678901234567890e0"));
        assert!(dec("12345678901234567890.0"));
        assert!(dec("1e400"));
        assert!(dec("1e-400"));
        assert!(!dec("1_000_000"));
        assert!(!dec("5e-324"));
        assert!(!dec("1.7976931348623157e308"));
        assert!(!dec("100000000000000000000000"));
        assert!(dec("123456789012345678901234"));
        assert!(!dec("0.000001"));
        assert!(!dec("12300"));
        assert!(!dec("0"));
        assert!(!dec("0.0"));
        assert!(!dec(".5"));
        assert!(!dec("5."));
        assert!(!dec("3.141592653589793"));
        assert!(!dec("3.14159265358979323846"));
        assert!(!dec("2e-7"));
    }

    #[test]
    fn radix_literals() {
        assert_eq!(radix_loses_precision("20000000000001", 4), Some(true));
        assert_eq!(radix_loses_precision("1fffffffffffff", 4), Some(false));
        assert_eq!(radix_loses_precision("FFFFFFFFFFFFFFFF", 4), Some(true));
        assert_eq!(radix_loses_precision("10000000000000000", 4), Some(false));
        assert_eq!(radix_loses_precision("0", 4), Some(false));
        assert_eq!(radix_loses_precision("777", 3), Some(false));
        assert_eq!(radix_loses_precision("1010", 1), Some(false));
        assert_eq!(strip_prefix("0x1F"), "1F");
        assert_eq!(strip_prefix("017"), "017");
    }
}
