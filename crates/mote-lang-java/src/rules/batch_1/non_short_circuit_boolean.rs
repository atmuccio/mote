//! `java/non-short-circuit-boolean` — `a & b` / `a | b` on booleans.

use super::util::{self, Fld, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["suspicious", "null-safety"],
    remediation_minutes: 2,
    description: r#"Detects the non-short-circuit operators `&` and `|` applied to two `boolean`
operands.

**Why it matters.** Unlike `&&` and `||`, these operators always evaluate the right-hand side.
Guards such as `s != null & s.isEmpty()` then throw exactly when the guard was supposed to
protect, and expensive right-hand sides run needlessly. When both sides really must run (for
their side effects) the intent is invisible to the reader.

**How to fix.** Use `&&` / `||`. If both operands must be evaluated, compute them into local
variables first and combine those.

```java
// Noncompliant
if (user != null & user.isActive()) { ... }

// Compliant
if (user != null && user.isActive()) { ... }
```

Compound assignments (`ok &= step()`, `changed |= update()`) are not reported; neither are
integer bit operations or operands whose type is unknown."#,
    ..RuleMeta::new(
        "java/non-short-circuit-boolean",
        "java",
        "Non-short-circuit boolean operator",
    )
};

pub struct NonShortCircuitBoolean;

impl<'t> JavaRule<'t> for NonShortCircuitBoolean {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["binary_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let op = ctx.operator_str(node);
        if op != "&" && op != "|" {
            return;
        }
        let (Some(left), Some(right)) = (node.fld(F::Left), node.fld(F::Right)) else {
            return;
        };
        if util::too_deep(ctx, node) {
            return;
        }
        if !ctx.type_of(left).is_some_and(|t| t.is_boolean_like()) {
            return;
        }
        if !ctx.type_of(right).is_some_and(|t| t.is_boolean_like()) {
            return;
        }
        let better = if op == "&" { "&&" } else { "||" };
        let at = util::op(node).unwrap_or(node);
        ctx.emit(
            at,
            format!("Use `{better}` instead of `{op}`; `{op}` always evaluates both operands"),
        );
    }
}
