//! `java/equals-with-null` — `x.equals(null)`.

use super::util::{self, Fld, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["null-safety", "equality"],
    remediation_minutes: 2,
    description: r#"Detects `x.equals(null)`.

**Why it matters.** By the `equals` contract, `x.equals(null)` is always `false` for a
non-null `x` — and when `x` *is* null the call throws a `NullPointerException` before
`equals` even runs. So the expression can never detect a null value; the intended check was
almost certainly `x == null`.

**How to fix.** Compare the reference: `x == null` (or `Objects.isNull(x)`).

```java
// Noncompliant
if (result.equals(null)) { return fallback; }

// Compliant
if (result == null) { return fallback; }
```

Test code is not checked: `assertFalse(value.equals(null))` legitimately verifies the
`equals` contract."#,
    ..RuleMeta::new("java/equals-with-null", "java", "equals() called with null")
};

pub struct EqualsWithNull;

impl<'t> JavaRule<'t> for EqualsWithNull {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["equals"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        // Text pre-filter (no tree access): the call ends with `null)`.
        let text = ctx
            .text(node)
            .trim_end_matches(|c: char| c == ')' || c.is_whitespace());
        if !text.ends_with("null") {
            return;
        }
        if util::method_name(ctx, node) != "equals" || ctx.is_test() {
            return;
        }
        let mut args = util::args(node);
        let (Some(arg), None) = (args.next(), args.next()) else {
            return;
        };
        if !syntax::is_null_literal(arg) {
            return;
        }
        let Some(recv) = util::plain_receiver(node) else {
            return;
        };
        // `Objects.equals(..)` style static helpers take two arguments; a type receiver with one
        // argument is not the instance method.
        if ctx.type_name_of(recv).is_some() {
            return;
        }
        let at = node.fld(F::Name).unwrap_or(node);
        ctx.emit(
            at,
            "Replace `.equals(null)` with `== null`; this call is always false or throws",
        );
    }
}
