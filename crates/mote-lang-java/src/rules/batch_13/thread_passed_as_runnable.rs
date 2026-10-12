//! `java/thread-passed-as-runnable` — a `Thread` object handed to a `Thread` constructor or an
//! executor, which only call its `run()`.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["concurrency", "thread"],
    remediation_minutes: 5,
    description: r#"Detects a `Thread` object (a `new Thread(..)`, a variable declared as `Thread` or
a subclass of it) passed where only a `Runnable` is needed: to a `Thread` constructor, to
`Executor.execute`, `ExecutorService.submit`, `ScheduledExecutorService.schedule…` or
`CompletableFuture.runAsync`.

**Why it matters.** The receiver only calls the object's `run()` method on a thread of its
own. The `Thread` object itself is never started: its name, priority, daemon flag,
uncaught-exception handler and thread-local state are silently ignored, and `join()`,
`interrupt()` or `isAlive()` on it do nothing useful. Readers (and the author) usually believe
those settings apply.

**How to fix.** Pass a `Runnable` (a lambda or the task object) instead, and configure the
thread through the executor's `ThreadFactory`; or start the thread yourself.

```java
// Noncompliant
Thread worker = new Thread(this::poll, "poller");
worker.setDaemon(true);
executor.execute(worker);        // runs poll(), not as "poller", not as a daemon

// Compliant
executor.execute(this::poll);
```

Only arguments whose type is provably `Thread` or a subclass are reported; executors are
recognized by their declared JDK types."#,
    ..RuleMeta::new(
        "java/thread-passed-as-runnable",
        "java",
        "Thread passed where a Runnable is expected",
    )
};

#[derive(Default)]
pub struct ThreadPassedAsRunnable {
    gate: util::Gate,
}

const CALLS: &[&str] = &[
    "execute",
    "submit",
    "schedule",
    "scheduleAtFixedRate",
    "scheduleWithFixedDelay",
    "runAsync",
];

impl<'t> JavaRule<'t> for ThreadPassedAsRunnable {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["object_creation_expression", "method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        CALLS
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if kind(node) == "object_creation_expression" {
            let Some(ty) = node.field(Field::Type) else {
                return;
            };
            if !matches!(ctx.text(ty), "Thread" | "java.lang.Thread") {
                return;
            }
            let resolved = ctx.resolve_type(ty);
            if resolved.as_ref().and_then(|t| t.fqn()) != Some("java.lang.Thread") {
                return;
            }
            // Only the `Runnable` parameter of a Thread constructor accepts a Thread.
            for a in util::args(node) {
                if is_thread(ctx, a) {
                    ctx.emit(
                        a,
                        "Pass a Runnable instead of this Thread: the new thread only calls its `run()`, and its own settings are ignored",
                    );
                }
            }
            return;
        }
        let Some(name) = ctx.call_name(node) else {
            return;
        };
        if !CALLS.contains(&name) {
            return;
        }
        let Some(first) = util::arg(node, 0) else {
            return;
        };
        if !matches!(
            kind(syntax::strip_parens(first)),
            "identifier" | "object_creation_expression" | "field_access" | "this"
        ) || !self.gate.open(ctx, "Thread")
        {
            return;
        }
        let Some(recv) = node.field(Field::Object) else {
            return;
        };
        let executor = if name == "runAsync" {
            util::names_type(ctx, recv, "java.util.concurrent.CompletableFuture")
        } else {
            util::is_a(ctx, recv, "java.util.concurrent.Executor")
        };
        if executor && is_thread(ctx, first) {
            ctx.emit(
                first,
                format!("Pass a Runnable instead of this Thread: `{name}` only calls its `run()`, and the Thread's own settings are ignored"),
            );
        }
    }
}

fn is_thread<'t>(ctx: &Ctx<'t>, expr: Node<'t>) -> bool {
    let e = syntax::strip_parens(expr);
    if !matches!(
        kind(e),
        "identifier" | "object_creation_expression" | "field_access" | "this"
    ) {
        return false;
    }
    util::is_a(ctx, e, "java.lang.Thread")
}
