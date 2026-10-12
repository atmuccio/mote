//! `java/byte-sign-extension` — a `byte` combined into a wider value without `& 0xFF`, and
//! `read()` implementations returning a raw `byte`.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["numbers", "bitwise", "io"],
    remediation_minutes: 2,
    description: r#"Detects two ways a signed `byte` corrupts a wider value:

* a `byte` OR-ed or added, unmasked, into a value assembled from shifted parts:
  `(b[0] << 8) | b[1]`, `(hi << 16) + lo` with `lo` a `byte`;
* an `int read()` method (the `InputStream` contract) returning a `byte` expression directly:
  `return buffer[pos++];`.

**Why it matters.** Bytes are signed. Before `|` or `+`, a byte is widened to `int` with
sign extension, so `0x80`–`0xFF` become `0xFFFFFF80`–`0xFFFFFFFF`, and OR-ing that sets every
higher bit of the result: `(0x12 << 8) | (byte) 0x80` is `-128`, not `0x1280`. An
`InputStream.read()` must return `0`–`255` or `-1` for end of stream; returning a raw byte
reports `-1` (end of data) for the byte `0xFF` and negative values for all bytes above `0x7F`.

**How to fix.** Mask the byte: `b & 0xFF`.

```java
// Noncompliant
int length = (header[0] << 8) | header[1];
public int read() { return data[pos++]; }

// Compliant
int length = ((header[0] & 0xFF) << 8) | (header[1] & 0xFF);
public int read() { return data[pos++] & 0xFF; }
```

Only operands whose type the resolver proves to be `byte` are reported, and only in an
expression that also contains a left shift by 8 or more (a value being assembled from bytes).
The high byte of such an expression (the shifted one) is not reported: its sign often
intentionally makes the result negative."#,
    ..RuleMeta::new(
        "java/byte-sign-extension",
        "java",
        "Byte sign-extended into a wider value",
    )
};

#[derive(Default)]
pub struct ByteSignExtension {
    shifts: util::Gate,
    reads: util::Gate,
}

/// Whether `node` is a left shift by 8 or more, or an `|` / `+` chain containing one.
fn assembles_bytes(ctx: &Ctx<'_>, node: Node<'_>, depth: u32) -> bool {
    if depth > 16 {
        return false;
    }
    let node = syntax::strip_parens(node);
    if kind(node) != "binary_expression" {
        return false;
    }
    match syntax::operator_str(node) {
        "<<" => node
            .field(Field::Right)
            .and_then(|r| syntax::int_literal_value(r, ctx.source()))
            .is_some_and(|d| d >= 8),
        "|" | "+" => {
            node.field(Field::Left)
                .is_some_and(|l| assembles_bytes(ctx, l, depth + 1))
                || node
                    .field(Field::Right)
                    .is_some_and(|r| assembles_bytes(ctx, r, depth + 1))
        }
        _ => false,
    }
}

/// Whether `expr` is a plain `byte`-typed expression (not masked, not shifted).
fn is_raw_byte<'t>(ctx: &Ctx<'t>, expr: Node<'t>) -> bool {
    let expr = syntax::strip_parens(expr);
    matches!(
        kind(expr),
        "identifier" | "array_access" | "method_invocation" | "field_access"
    ) && ctx
        .type_of(expr)
        .is_some_and(|t| t.prim() == Some(Prim::Byte))
}

impl<'t> JavaRule<'t> for ByteSignExtension {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["binary_expression", "return_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if kind(node) == "return_statement" {
            self.check_read(node, ctx);
            return;
        }
        let Some(op) = ctx.operator(node) else {
            return;
        };
        if !matches!(kind(op), "|" | "+") || !self.shifts.open(ctx, "<<") {
            return;
        }
        if !ctx.text(node).contains("<<") {
            return;
        }
        let (Some(left), Some(right)) = (node.field(Field::Left), node.field(Field::Right)) else {
            return;
        };
        for (raw, other) in [(right, left), (left, right)] {
            let r = syntax::strip_parens(raw);
            if !matches!(
                kind(r),
                "identifier" | "array_access" | "method_invocation" | "field_access"
            ) {
                continue;
            }
            if !assembles_bytes(ctx, other, 0) || !is_raw_byte(ctx, r) {
                continue;
            }
            // `+` may be a string concatenation: require an integral result.
            if kind(op) == "+"
                && !ctx
                    .type_of(node)
                    .and_then(|t| t.prim())
                    .is_some_and(|p| p.is_integral())
            {
                return;
            }
            ctx.emit(
                raw,
                format!(
                    "Mask this byte with `& 0xFF`: `{}` is sign-extended, so values above 0x7F set all higher bits",
                    ctx.text(r)
                ),
            );
            return;
        }
    }
}

impl ByteSignExtension {
    fn check_read<'t>(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !self.reads.open(ctx, "int read()") {
            return;
        }
        let Some(expr) = syntax::first_named_child(node) else {
            return;
        };
        let e = syntax::strip_parens(expr);
        if !matches!(kind(e), "array_access" | "identifier" | "method_invocation") {
            return;
        }
        let Some(method) = ctx.enclosing_function(node) else {
            return;
        };
        if kind(method) != "method_declaration"
            || syntax::name_of(method, ctx.source()) != "read"
            || syntax::parameters(method).next().is_some()
            || method.field(Field::Type).map(|t| ctx.text(t)) != Some("int")
        {
            return;
        }
        if !is_raw_byte(ctx, e) {
            return;
        }
        ctx.emit(
            expr,
            "Return `b & 0xFF`: `read()` must return 0-255 or -1, but a raw byte is negative above 0x7F (and 0xFF reads as end of stream)",
        );
    }
}
