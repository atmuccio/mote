//! `java/shift-out-of-range` — `x << 32` on an `int`, `x >> 64` on a `long`, negative
//! shift distances.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["numbers", "bitwise"],
    remediation_minutes: 2,
    description: r#"Detects shifts (`<<`, `>>`, `>>>`, and their compound assignments) by a constant
distance that is negative or not smaller than the width of the shifted value: 32 for `int`
(and `short`, `byte`, `char`, which are promoted to `int`), 64 for `long`.

**Why it matters.** Java uses only the low 5 (or 6) bits of the distance: `x << 32` on an
`int` shifts by 0 and returns `x` unchanged, `1 << 40` is `256`, and `x >> -1` shifts by 31.
The author expected the bits to move out (or a 64-bit result) and gets something else,
silently.

**How to fix.** Shift a `long` when more than 32 bits are needed (`1L << 40`), or correct the
distance.

```java
// Noncompliant
long mask = 1 << 40;
int high = value >>> 32;

// Compliant
long mask = 1L << 40;
long high = longValue >>> 32;
```

Only constant distances are checked, and only when the type of the shifted value is known."#,
    ..RuleMeta::new(
        "java/shift-out-of-range",
        "java",
        "Shift distance out of range",
    )
};

pub struct ShiftOutOfRange;

impl<'t> JavaRule<'t> for ShiftOutOfRange {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["binary_expression", "assignment_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(op) = ctx.operator(node) else {
            return;
        };
        if !matches!(kind(op), "<<" | ">>" | ">>>" | "<<=" | ">>=" | ">>>=") {
            return;
        }
        let (Some(left), Some(right)) = (node.field(Field::Left), node.field(Field::Right)) else {
            return;
        };
        let Some(dist) = util::int_value(ctx, right) else {
            return;
        };
        if (0..32).contains(&dist) {
            return;
        }
        let width = match ctx.type_of(left).and_then(|t| t.unboxed()) {
            Some(Prim::Long) => 64,
            Some(Prim::Int | Prim::Short | Prim::Byte | Prim::Char) => 32,
            _ => return,
        };
        if (0..width).contains(&dist) {
            return;
        }
        let ty = if width == 64 { "long" } else { "int" };
        let effective = dist.rem_euclid(width);
        let fix = if width == 32 && dist > 0 {
            " (use a `long` operand)"
        } else {
            ""
        };
        ctx.emit(
            right,
            format!("Fix the shift distance: an `{ty}` shift by {dist} actually shifts by {effective}{fix}"),
        );
    }
}
