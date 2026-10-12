//! `java/scheduled-pool-without-core-threads` — a scheduled executor with zero core threads.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["concurrency", "executors"],
    remediation_minutes: 5,
    description: r#"Detects scheduled executors configured with a core pool size of 0:
`Executors.newScheduledThreadPool(0)`, `new ScheduledThreadPoolExecutor(0, …)`, and
`setCorePoolSize(0)` on a `ScheduledThreadPoolExecutor`.

**Why it matters.** A `ScheduledThreadPoolExecutor` is a fixed-size pool: its maximum size
has no effect, and it only keeps core threads alive while waiting for delayed tasks. With no
core thread, the pool falls back on a temporary worker that keeps timing out while it waits
for the next due task: on Java 8 this spins a CPU at 100% (a known JDK bug), on later versions
the worker still wakes up every few milliseconds, and the class documentation warns that the
pool may be left without a thread to run tasks once they become due. It is never what the
author wanted from "an idle pool".

**How to fix.** Use at least one core thread (the pool never grows beyond the core size
anyway).

```java
// Noncompliant
ScheduledExecutorService timer = Executors.newScheduledThreadPool(0);

// Compliant
ScheduledExecutorService timer = Executors.newSingleThreadScheduledExecutor();
```

Only the literal `0` is reported, and not a `setCorePoolSize(0)` on a pool that the same method
has already shut down (`pool.shutdown(); pool.setCorePoolSize(0);` is teardown that lets idle
threads exit, not a live pool)."#,
    ..RuleMeta::new(
        "java/scheduled-pool-without-core-threads",
        "java",
        "Scheduled executor without core threads",
    )
};

#[derive(Default)]
pub struct ScheduledPoolWithoutCoreThreads {
    scheduled: util::Gate,
}

const MSG: &str = "Use at least one core thread: a scheduled pool with 0 core threads busy-polls for due tasks and may have no thread to run them";

impl<'t> JavaRule<'t> for ScheduledPoolWithoutCoreThreads {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation", "object_creation_expression"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["newScheduledThreadPool", "setCorePoolSize"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !self.scheduled.open(ctx, "Scheduled") {
            return;
        }
        let Some(first) = util::arg(node, 0) else {
            return;
        };
        if util::int_value(ctx, first) != Some(0) {
            return;
        }
        let proven = if kind(node) == "object_creation_expression" {
            node.field(Field::Type).is_some_and(|ty| {
                util::last_segment(ctx.text(ty)) == "ScheduledThreadPoolExecutor"
                    && ctx.resolve_type_name(ctx.text(ty), ty).as_deref()
                        == Some("java.util.concurrent.ScheduledThreadPoolExecutor")
            })
        } else {
            match ctx.call_name(node) {
                Some("newScheduledThreadPool") => node
                    .field(Field::Object)
                    .is_some_and(|o| util::names_type(ctx, o, "java.util.concurrent.Executors")),
                Some("setCorePoolSize") => {
                    util::receiver_is(
                        ctx,
                        node,
                        &["java.util.concurrent.ScheduledThreadPoolExecutor"],
                    ) && !after_shutdown(ctx, node)
                }
                _ => false,
            }
        };
        if proven {
            ctx.emit(first, MSG);
        }
    }
}

/// Whether the pool was shut down earlier in the same method (`pool.shutdown();` …
/// `pool.setCorePoolSize(0);`): dropping the core threads is then teardown, letting idle
/// threads exit, not a live pool left without a thread.
fn after_shutdown<'t>(ctx: &Ctx<'t>, call: Node<'t>) -> bool {
    let Some(recv) = call.field(Field::Object).map(|o| ctx.text(o)) else {
        return false;
    };
    let Some(body) = ctx
        .enclosing_function(call)
        .and_then(|f| f.field(Field::Body))
    else {
        return false;
    };
    let Some(before) = ctx.source().get(body.start_byte()..call.start_byte()) else {
        return false;
    };
    let recv = recv.strip_prefix("this.").unwrap_or(recv);
    // `recv.shutdown()` / `recv.shutdownNow()` / `this.recv.shutdown()`, as a whole name.
    before.match_indices(".shutdown").any(|(i, _)| {
        before[..i].trim_end().strip_suffix(recv).is_some_and(|h| {
            h.ends_with("this.")
                || !h.ends_with(|c: char| c.is_alphanumeric() || matches!(c, '_' | '$' | '.'))
        })
    })
}
