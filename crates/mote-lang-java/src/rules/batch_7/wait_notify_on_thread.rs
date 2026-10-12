//! `java/wait-notify-on-thread` — `wait`/`notify`/`notifyAll` on a `Thread` object.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["concurrency", "multi-threading"],
    remediation_minutes: 15,
    description: r#"Detects `wait()`, `notify()` and `notifyAll()` called on a `Thread` object
(including unqualified calls inside a `Thread` subclass).

**Why it matters.** The JVM uses the monitor of a `Thread` object itself: `Thread.join()` is
implemented by waiting on it, and the thread notifies all waiters on it when it terminates.
Using the same monitor for your own signaling mixes both protocols — your waiters wake up when
the thread ends, and your notifications can wake up joiners — which leads to spurious
wake-ups and lost signals that are very hard to diagnose. The `Thread` documentation
recommends never doing it.

**How to fix.** Signal through a dedicated lock object, or use `join()`, a `CountDownLatch`,
or a `Future` to wait for the thread.

```java
// Noncompliant
synchronized (worker) {
    worker.wait();
}

// Compliant
worker.join();
```

Only receivers the resolver proves to be a `java.lang.Thread` are reported."#,
    ..RuleMeta::new(
        "java/wait-notify-on-thread",
        "java",
        "wait/notify on a Thread object",
    )
};

pub struct WaitNotifyOnThread;

impl<'t> JavaRule<'t> for WaitNotifyOnThread {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["wait", "notify", "notifyAll"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) = ctx.call_name(node) else {
            return;
        };
        if !matches!(name, "wait" | "notify" | "notifyAll") {
            return;
        }
        // `this.wait()` in a Thread subclass is the same as the unqualified call.
        let on_thread = match node.field(Field::Object) {
            Some(o) if kind(o) == "super" => false,
            Some(o) if kind(o) == "this" => ctx
                .enclosing_class(node)
                .and_then(|c| ctx.class_type(c))
                .is_some_and(|t| ctx.is_subtype_of(&t, "java.lang.Thread")),
            _ => ctx.is_call_to(node, "java.lang.Thread", name),
        };
        if !on_thread {
            return;
        }
        ctx.emit(
            node.field(Field::Name).unwrap_or(node),
            format!("Don't call `{name}()` on a Thread: the JVM uses its monitor for join() and termination; signal through a separate lock object"),
        );
    }
}
