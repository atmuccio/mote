//! `java/atomic-equals` — `AtomicInteger.equals(..)` compares identities, not values.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["equality", "concurrency"],
    remediation_minutes: 2,
    description: r#"Detects `equals(..)` called on an `AtomicInteger`, `AtomicLong`, or
`AtomicBoolean`.

**Why it matters.** The atomic wrappers are mutable holders and do not override `equals`
(or `hashCode`): `new AtomicInteger(1).equals(new AtomicInteger(1))` is `false`, and
`counter.equals(5)` is always `false`. Code that looks like a value comparison is really an
identity check.

**How to fix.** Compare the current values: `a.get() == b.get()`, or
`counter.get() == 5`.

```java
// Noncompliant
if (retries.equals(maxRetries)) { giveUp(); }

// Compliant
if (retries.get() == maxRetries.get()) { giveUp(); }
```

Only receivers whose type the resolver proves to be one of the three classes are
reported."#,
    ..RuleMeta::new(
        "java/atomic-equals",
        "java",
        "equals() on an atomic number or boolean",
    )
};

#[derive(Default)]
pub struct AtomicEquals {
    atomic_in_file: util::Gate,
}

impl<'t> JavaRule<'t> for AtomicEquals {
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
        if !self.atomic_in_file.open(ctx, "java.util.concurrent.atomic") {
            return;
        }
        let Some(name) = util::fld(node, F::Name) else {
            return;
        };
        if ctx.text(name) != "equals" || util::arg_count(node) != 1 {
            return;
        }
        let Some(recv) = util::fld(node, F::Object) else {
            return;
        };
        if matches!(kind(recv), "this" | "super" | "string_literal") {
            return;
        }
        let Some(t) = ctx.type_of(recv) else {
            return;
        };
        let simple = match t.fqn() {
            Some("java.util.concurrent.atomic.AtomicInteger") => "AtomicInteger",
            Some("java.util.concurrent.atomic.AtomicLong") => "AtomicLong",
            Some("java.util.concurrent.atomic.AtomicBoolean") => "AtomicBoolean",
            _ => return,
        };
        ctx.emit(
            name,
            format!("Compare the values with get(): `{simple}` does not override equals(), so this compares object identity"),
        );
    }
}
