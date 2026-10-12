//! `java/thread-run-direct` — `thread.run()` instead of `thread.start()`.

use super::util::{self, Fld, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["concurrency"],
    remediation_minutes: 2,
    description: r#"Detects calls to `run()` on an expression whose type is `java.lang.Thread` (or
a subclass).

**Why it matters.** `run()` is an ordinary method: calling it executes the thread's task
synchronously in the *current* thread. No new thread is started, so the caller blocks and any
intended concurrency silently disappears.

**How to fix.** Call `start()`, which creates the thread and invokes `run()` on it. If the work
really should run inline, call the `Runnable` directly rather than wrapping it in a `Thread`.

```java
// Noncompliant
Thread worker = new Thread(task);
worker.run();

// Compliant
Thread worker = new Thread(task);
worker.start();
```

Only calls on a receiver whose type is provably a `Thread` are reported; `super.run()` and
unqualified `run()` calls inside a `Thread` subclass are not."#,
    ..RuleMeta::new(
        "java/thread-run-direct",
        "java",
        "Thread.run() called directly",
    )
};

pub struct ThreadRunDirect;

impl<'t> JavaRule<'t> for ThreadRunDirect {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["run"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        // Text pre-filter (no tree access).
        if !ctx.text(node).ends_with("run()") {
            return;
        }
        if util::method_name(ctx, node) != "run" || util::arg_count(node) != 0 {
            return;
        }
        let Some(recv) = util::plain_receiver(node) else {
            return;
        };
        let Some(t) = ctx.type_of(recv) else { return };
        if !ctx.is_subtype_of(&t, "java.lang.Thread") {
            return;
        }
        let at = node.fld(F::Name).unwrap_or(node);
        ctx.emit(
            at,
            "Call start() to run this thread; run() executes it in the current thread",
        );
    }
}
