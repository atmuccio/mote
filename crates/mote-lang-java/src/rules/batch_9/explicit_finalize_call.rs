//! `java/explicit-finalize-call` — calling `finalize()` directly, or enabling finalizers on exit.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["finalization", "resources"],
    remediation_minutes: 10,
    description: r#"Detects explicit calls to `finalize()` (other than `super.finalize()`) and
calls to `System.runFinalizersOnExit(..)` / `Runtime.runFinalizersOnExit(..)`.

**Why it matters.** `finalize()` is the garbage collector's hook: calling it by hand does not
free the object, and the collector will still call it again later, so the cleanup runs twice
— closing a handle twice, releasing a native resource that another object now owns.
`runFinalizersOnExit` was deprecated from the start because it can run finalizers on live
objects while other threads still use them; it was removed in Java 11.

**How to fix.** Move the cleanup into a `close()` method (implement `AutoCloseable` and use
try-with-resources) and call that; use `java.lang.ref.Cleaner` as a safety net.

```java
// Noncompliant
connection.finalize();
System.runFinalizersOnExit(true);

// Compliant
connection.close();
```

Not reported: `super.finalize()` (the normal chaining inside a finalizer) and test code,
which may call a finalizer on purpose to exercise it."#,
    ..RuleMeta::new(
        "java/explicit-finalize-call",
        "java",
        "Explicit call to finalize()",
    )
};

pub struct ExplicitFinalizeCall;

impl<'t> JavaRule<'t> for ExplicitFinalizeCall {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["finalize", "runFinalizersOnExit"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) = ctx.call_name(node) else {
            return;
        };
        if ctx.is_test() {
            return;
        }
        let obj = node.field(Field::Object);
        let n = util::arg_count(node);
        match name {
            "finalize" => {
                // Any zero-argument `finalize()` is `Object.finalize` or an override of it.
                if n != 0 || obj.is_some_and(|o| kind(o) == "super") {
                    return;
                }
                let at = node.field(Field::Name).unwrap_or(node);
                ctx.emit(
                    at,
                    "Do not call `finalize()` directly: the garbage collector calls it again later; move the cleanup to `close()`",
                );
            }
            "runFinalizersOnExit" => {
                let Some(obj) = obj else {
                    return;
                };
                if n != 1
                    || !(util::names_type(ctx, obj, "java.lang.System")
                        || util::names_type(ctx, obj, "java.lang.Runtime")
                        || ctx
                            .type_of(obj)
                            .is_some_and(|t| t.is_class("java.lang.Runtime")))
                {
                    return;
                }
                let at = node.field(Field::Name).unwrap_or(node);
                ctx.emit(
                    at,
                    "Remove this call: running finalizers on exit is unsafe (finalizers may run on objects still in use) and was removed in Java 11",
                );
            }
            _ => {}
        }
    }
}
