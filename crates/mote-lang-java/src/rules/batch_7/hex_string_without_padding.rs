//! `java/hex-string-without-padding` — `Integer.toHexString(b & 0xFF)` appended as a byte's hex
//! digits, losing the leading zero.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["strings", "encoding"],
    remediation_minutes: 5,
    description: r#"Detects bytes converted with `Integer.toHexString(b & 0xFF)` (or
`Long.toHexString`) and appended back to back to a string or builder inside a loop, with no
padding to two digits.

**Why it matters.** `toHexString` drops leading zeros: the byte `0x0A` becomes `"a"`, not
`"0a"`. Hex strings built this way — digests, checksums, tokens, encoded keys — have a variable
length and are ambiguous (`0x01 0x23` and `0x12 0x03` both give `"123"`), so comparisons with
correctly encoded values fail for some inputs only.

**How to fix.** Pad each byte to two digits, or use a formatter.

```java
// Noncompliant
for (byte b : digest) {
    hex.append(Integer.toHexString(b & 0xFF));
}

// Compliant
for (byte b : digest) {
    hex.append(String.format("%02x", b));
}
String same = HexFormat.of().formatHex(digest);   // Java 17+
```

Not reported: results stored in a variable first (usually padded next), conversions outside
loops, and loop bodies that contain any string or character literal or check a `length()` —
the usual manual padding, or separators such as `':'` that keep the output unambiguous."#,
    ..RuleMeta::new(
        "java/hex-string-without-padding",
        "java",
        "Byte converted to hex without zero padding",
    )
};

pub struct HexStringWithoutPadding;

/// Whether `expr` is `x & 0xFF` / `0xFF & x`.
fn is_byte_mask(ctx: &Ctx<'_>, expr: Node<'_>) -> bool {
    let expr = syntax::strip_parens(expr);
    if kind(expr) != "binary_expression" || syntax::operator_str(expr) != "&" {
        return false;
    }
    [Field::Left, Field::Right]
        .into_iter()
        .filter_map(|f| expr.field(f))
        .any(|side| util::int_value(ctx, side) == Some(0xff))
}

impl<'t> JavaRule<'t> for HexStringWithoutPadding {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["toHexString"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if ctx.call_name(node) != Some("toHexString") {
            return;
        }
        let mut args = util::args(node);
        let (Some(arg), None) = (args.next(), args.next()) else {
            return;
        };
        if !is_byte_mask(ctx, arg) {
            return;
        }
        let Some(recv) = node.field(Field::Object) else {
            return;
        };
        if !util::names_type(ctx, recv, "java.lang.Integer")
            && !util::names_type(ctx, recv, "java.lang.Long")
        {
            return;
        }
        // Climb through case conversions to the value's use.
        let mut value = node;
        let mut parent = ctx.parent(node);
        while let Some(p) = parent {
            if kind(p) == "method_invocation"
                && matches!(ctx.call_name(p), Some("toUpperCase" | "toLowerCase"))
                && p.field(Field::Object).is_some_and(|o| o.id() == value.id())
            {
                value = p;
                parent = ctx.parent(p);
            } else {
                break;
            }
        }
        let Some(p) = parent else {
            return;
        };
        let appended = match kind(p) {
            "argument_list" => ctx
                .parent(p)
                .is_some_and(|call| ctx.call_name(call) == Some("append")),
            "binary_expression" => syntax::operator_str(p) == "+",
            "assignment_expression" => syntax::operator_str(p) == "+=",
            _ => false,
        };
        if !appended {
            return;
        }
        // Only bytes concatenated back to back in a loop: the loop body appends no literal
        // (a padding "0" or a separator such as ':' makes the output unambiguous) and checks no
        // length.
        let Some(lp) = ctx.ancestors(node).find(|a| {
            matches!(
                kind(*a),
                "for_statement"
                    | "enhanced_for_statement"
                    | "while_statement"
                    | "do_statement"
                    | "method_declaration"
                    | "lambda_expression"
                    | "class_body"
            )
        }) else {
            return;
        };
        let Some(loop_body) = lp.field(Field::Body) else {
            return;
        };
        if matches!(
            kind(lp),
            "method_declaration" | "lambda_expression" | "class_body"
        ) {
            return;
        }
        let text = ctx.text(loop_body);
        if text.contains('"') || text.contains('\'') || text.contains("length()") {
            return;
        }
        ctx.emit(
            node,
            "Pad to two digits (e.g. `String.format(\"%02x\", b)`): `toHexString` drops the leading zero of bytes below 0x10",
        );
    }
}
