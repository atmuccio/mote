//! `java/sleep-in-synchronized` — `Thread.sleep` while holding a monitor.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["concurrency", "multi-threading", "performance"],
    remediation_minutes: 15,
    description: r#"Detects `Thread.sleep(..)` (and `TimeUnit.X.sleep(..)`) called inside a
`synchronized` block or a `synchronized` method.

**Why it matters.** Sleeping does not release monitors. Every other thread that needs the same
lock — often every caller of the object — is blocked for the whole sleep, turning a pause in
one thread into a stall of the application, and making deadlocks more likely when the sleeper
waits for another thread to make progress.

**How to fix.** Sleep outside the synchronized region, or wait on the monitor with a timeout
(`lock.wait(millis)` releases it while waiting), or use a `ScheduledExecutorService` for
delayed work.

```java
// Noncompliant
synchronized (queue) {
    while (queue.isEmpty()) {
        Thread.sleep(100);
    }
}

// Compliant
synchronized (queue) {
    while (queue.isEmpty()) {
        queue.wait(100);
    }
}
```

Test code (which sometimes holds a lock on purpose to provoke contention) and sleeps inside
lambdas or anonymous classes declared in the synchronized region (they run later, elsewhere)
are not reported."#,
    ..RuleMeta::new(
        "java/sleep-in-synchronized",
        "java",
        "Thread.sleep while holding a lock",
    )
};

pub struct SleepInSynchronized;

impl<'t> JavaRule<'t> for SleepInSynchronized {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["sleep"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if ctx.call_name(node) != Some("sleep") || ctx.is_test() {
            return;
        }
        // Find the lock holder first (walk-stack ancestors are cheap), then prove the call.
        let mut holder = None;
        for a in ctx.ancestors(node) {
            match kind(a) {
                "synchronized_statement" => {
                    holder = Some(a);
                    break;
                }
                "method_declaration" => {
                    if syntax::has_modifier(a, "synchronized") {
                        holder = Some(a);
                    }
                    break;
                }
                "lambda_expression"
                | "class_body"
                | "constructor_declaration"
                | "static_initializer"
                | "program" => break,
                _ => {}
            }
        }
        let Some(holder) = holder else {
            return;
        };
        let proven = ctx.is_call_to(node, "java.lang.Thread", "sleep")
            || node.field(Field::Object).is_some_and(|o| {
                ctx.type_of(o)
                    .is_some_and(|t| t.is_class("java.util.concurrent.TimeUnit"))
            });
        if !proven {
            return;
        }
        let what = if kind(holder) == "synchronized_statement" {
            "this synchronized block"
        } else {
            "this synchronized method"
        };
        ctx.emit(
            node.field(Field::Name).unwrap_or(node),
            format!("Don't sleep while holding a lock: every thread waiting for the monitor of {what} is blocked too; use wait(timeout) or sleep outside"),
        );
    }
}
