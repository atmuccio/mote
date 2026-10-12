//! `java/int-to-long-bits-to-double` — `Double.longBitsToDouble` given a 32-bit value.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["numbers", "floating-point"],
    remediation_minutes: 5,
    description: r#"Detects `Double.longBitsToDouble(x)` where `x` is an `int` (or a `short`,
`char`, `byte`, or their wrappers), including int literals written without the `L` suffix.

**Why it matters.** `longBitsToDouble` reads a 64-bit IEEE 754 pattern. An `int` is widened
first — zero-extended for non-negative values, sign-extended for negative ones — so the
sign, exponent and top of the mantissa end up in the wrong bits: the result is a tiny
subnormal number or NaN, not the intended value. The usual cause is a 32-bit float pattern
that should go through `Float.intBitsToFloat`, or a 64-bit constant missing its `L`.

**How to fix.** Use `Float.intBitsToFloat(bits)` for 32-bit patterns, or pass a `long`
(`0x3FF0000000000000L`).

```java
// Noncompliant
double d = Double.longBitsToDouble(buffer.getInt());
double one = Double.longBitsToDouble(0x3FF00000);

// Compliant
float f = Float.intBitsToFloat(buffer.getInt());
double one = Double.longBitsToDouble(0x3FF0000000000000L);
```

The literal `0` (the same bits either way) is not reported."#,
    ..RuleMeta::new(
        "java/int-to-long-bits-to-double",
        "java",
        "Double.longBitsToDouble given an int",
    )
};

pub struct IntToLongBitsToDouble;

impl<'t> JavaRule<'t> for IntToLongBitsToDouble {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["longBitsToDouble"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if ctx.call_name(node) != Some("longBitsToDouble") || util::arg_count(node) != 1 {
            return;
        }
        let Some(obj) = node.field(Field::Object) else {
            return;
        };
        let Some(arg) = util::arg(node, 0) else {
            return;
        };
        if syntax::int_literal_value(arg, ctx.source()) == Some(0) {
            return;
        }
        if !util::names_type(ctx, obj, "java.lang.Double") {
            return;
        }
        let Some(t) = ctx.type_of(arg) else {
            return;
        };
        let narrow = matches!(
            t.unboxed(),
            Some(Prim::Int | Prim::Short | Prim::Char | Prim::Byte)
        );
        if !narrow {
            return;
        }
        ctx.emit(
            arg,
            format!(
                "Pass a 64-bit `long` here: this `{t}` is widened before its bits are read, so the result is not the intended double; use `Float.intBitsToFloat` for 32-bit patterns"
            ),
        );
    }
}
