//! `java/random-cast-to-int` — `(int) Math.random()` is always 0.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["numbers", "random"],
    remediation_minutes: 2,
    description: r#"Detects `Math.random()`, `Random.nextDouble()` or `Random.nextFloat()` cast
directly to an integer type: `(int) Math.random()`.

**Why it matters.** These methods return a value in `[0, 1)`, which truncates to `0`. The
usual form is `(int) Math.random() * n`: the cast binds tighter than the multiplication, so the
expression is `0 * n` and the "random" number is always zero.

**How to fix.** Scale before casting, or ask for an integer directly.

```java
// Noncompliant
int slot = (int) Math.random() * buckets;

// Compliant
int slot = (int) (Math.random() * buckets);
int other = ThreadLocalRandom.current().nextInt(buckets);
```

Only calls whose receiver is `Math`/`StrictMath` or provably a `java.util.Random` are
reported; `nextGaussian()` (which is not bounded by 1) is not."#,
    ..RuleMeta::new(
        "java/random-cast-to-int",
        "java",
        "Random fraction cast to an integer",
    )
};

pub struct RandomCastToInt;

impl<'t> JavaRule<'t> for RandomCastToInt {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["cast_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let (Some(ty), Some(value)) = (node.field(Field::Type), node.field(Field::Value)) else {
            return;
        };
        if kind(ty) != "integral_type" {
            return;
        }
        let call = syntax::strip_parens(value);
        let Some(name) = ctx.call_name(call) else {
            return;
        };
        if !matches!(name, "random" | "nextDouble" | "nextFloat")
            || syntax::argument_count(call) != 0
        {
            return;
        }
        let Some(recv) = call.field(Field::Object) else {
            return;
        };
        let proven = if name == "random" {
            util::names_math(ctx, recv)
        } else {
            util::receiver_is(ctx, call, &["java.util.Random"])
        };
        if !proven {
            return;
        }
        ctx.emit(
            node,
            format!(
                "Scale before casting: `{}` is below 1, so casting it to `{}` always gives 0",
                ctx.text(call),
                ctx.text(ty)
            ),
        );
    }
}
