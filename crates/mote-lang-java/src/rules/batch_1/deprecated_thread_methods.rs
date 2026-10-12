//! `java/deprecated-thread-methods` — `Thread.stop()`, `suspend()`, `resume()`.

use super::util::{self, Fld, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["concurrency", "deprecated"],
    remediation_minutes: 30,
    description: r#"Detects calls to `stop()`, `suspend()`, and `resume()` on a `Thread`
(including `Thread.currentThread()`).

**Why it matters.** These methods are inherently unsafe and have been deprecated since Java
1.2; recent JDKs make them throw `UnsupportedOperationException`. `stop()` kills the thread at
an arbitrary point, releasing its locks while the objects they guard are half-updated.
`suspend()` keeps every lock the thread holds, so any other thread that needs one deadlocks
until a `resume()` that may never come.

**How to fix.** Make the thread cooperate: have it check a `volatile` flag or its interrupted
status (`Thread.interrupted()`) and exit on its own, and call `interrupt()` to ask it to stop.
Use `wait`/`notify`, `LockSupport`, or `java.util.concurrent` utilities to pause work.

```java
// Noncompliant
worker.stop();

// Compliant
worker.interrupt();     // the worker loop checks Thread.currentThread().isInterrupted()
```

Only calls whose receiver is provably a `Thread` are reported (unqualified calls inside a
`Thread` subclass included)."#,
    ..RuleMeta::new(
        "java/deprecated-thread-methods",
        "java",
        "Unsafe Thread.stop/suspend/resume",
    )
};

pub struct DeprecatedThreadMethods;

impl<'t> JavaRule<'t> for DeprecatedThreadMethods {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["stop", "suspend", "resume"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        // Text pre-filter (no tree access).
        let text = ctx.text(node);
        if !(text.ends_with("stop()") || text.ends_with("suspend()") || text.ends_with("resume()"))
        {
            return;
        }
        let name = util::method_name(ctx, node);
        if !matches!(name, "stop" | "suspend" | "resume") || util::arg_count(node) != 0 {
            return;
        }
        if node.fld(F::Object).is_some_and(|o| kind(o) == "super") {
            return;
        }
        if !ctx.is_call_to(node, "java.lang.Thread", name) {
            return;
        }
        let at = node.fld(F::Name).unwrap_or(node);
        let msg = if name == "stop" {
            "Replace Thread.stop() with cooperative cancellation (interrupt() or a volatile flag)"
        } else {
            "Replace Thread.suspend()/resume() with proper synchronization; they deadlock easily"
        };
        ctx.emit(at, msg);
    }
}
