//! `java/wait-without-loop` — `wait()` not guarded by a loop re-checking the condition.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["concurrency", "multi-threading"],
    remediation_minutes: 10,
    description: r#"Detects untimed `Object.wait()` calls, and untimed `await()` / `awaitUninterruptibly()`
calls on a `java.util.concurrent.locks.Condition`, that are not inside a loop in the same method.

**Why it matters.** A waiting thread can wake up without the condition it waits for being
true: spurious wake-ups are allowed by the specification, `notifyAll()` wakes threads
waiting for other conditions, and another thread may consume the state change first. Code
that waits once with `if (!ready) wait();` then proceeds on a false assumption.

**How to fix.** Always wait in a loop that re-checks the condition while holding the lock —
or use higher-level tools (`CountDownLatch`, `BlockingQueue`, `CompletableFuture`).

```java
// Noncompliant
synchronized Item take() throws InterruptedException {
    if (items.isEmpty()) {
        wait();
    }
    return items.remove(0);
}

// Compliant
synchronized Item take() throws InterruptedException {
    while (items.isEmpty()) {
        wait();
    }
    return items.remove(0);
}
```

Any enclosing `while`, `do`, or `for` loop of the same method or lambda counts as a guard.
Only the untimed forms are reported: a timed `wait(millis)` or `await(time, unit)` must already handle waking up
without the condition (the timeout) and is often a deliberate bounded wait. `await()` counts
only when the receiver is proven to be a `Condition` (a `CountDownLatch` or a barrier is not
woken spuriously)."#,
    ..RuleMeta::new("java/wait-without-loop", "java", "wait() outside a loop")
};

pub struct WaitWithoutLoop;

impl<'t> JavaRule<'t> for WaitWithoutLoop {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["wait", "await", "awaitUninterruptibly"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        // O(1) text test before touching the tree.
        let text = ctx.text(node);
        if !text.ends_with("wait()") && !text.ends_with("awaitUninterruptibly()") {
            return;
        }
        let Some(name) = util::fld(node, F::Name) else {
            return;
        };
        let method = ctx.text(name);
        if !matches!(method, "wait" | "await" | "awaitUninterruptibly") {
            return;
        }
        // Only the untimed `wait()`: `Object.wait()` is final, so every no-argument call is the
        // monitor method. A timed wait already has to cope with waking up without the condition
        // (the timeout), and is often used deliberately as a bounded wait.
        if util::arg_count(node) != 0 {
            return;
        }
        if util::fld(node, F::Object).is_some_and(|o| kind(o) == "super") {
            return;
        }
        for a in ctx.ancestors(node) {
            match kind(a) {
                "while_statement" | "do_statement" | "for_statement" | "enhanced_for_statement" => {
                    return
                }
                "method_declaration"
                | "constructor_declaration"
                | "compact_constructor_declaration"
                | "lambda_expression"
                | "static_initializer" => break,
                "class_body" | "interface_body" | "enum_body" | "enum_body_declarations" => break,
                _ => {}
            }
        }
        // `await()` is only the same contract on a `java.util.concurrent.locks.Condition`
        // (a latch or a barrier waits for good): the receiver's type must prove it.
        if method != "wait" {
            let condition = util::fld(node, F::Object)
                .and_then(|o| ctx.type_of(o))
                .is_some_and(|t| ctx.is_subtype_of(&t, "java.util.concurrent.locks.Condition"));
            if !condition {
                return;
            }
        }
        ctx.emit(
            name,
            format!("Call {method}() inside a loop that re-checks the awaited condition; wake-ups can be spurious or meant for another waiter"),
        );
    }
}
