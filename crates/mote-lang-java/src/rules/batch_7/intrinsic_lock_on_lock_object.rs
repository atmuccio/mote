//! `java/intrinsic-lock-on-lock-object` — `synchronized (lock)` on a `Lock`, `condition.wait()`
//! instead of `await()`.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["concurrency", "multi-threading"],
    remediation_minutes: 10,
    description: r#"Detects the built-in monitor operations applied to a
`java.util.concurrent.locks` object:

* `synchronized (lock) { … }` where `lock` is a `Lock`, `ReadWriteLock`, `StampedLock` or
  `Condition`;
* `wait()`, `notify()` or `notifyAll()` called on a `Lock` or a `Condition`.

**Why it matters.** A `ReentrantLock` and the object's built-in monitor are unrelated.
`synchronized (lock)` does not acquire the lock, so it excludes nothing with code that calls
`lock.lock()`. `condition.wait()` / `condition.notify()` use the monitor of the `Condition`
object, not the condition itself: without `synchronized (condition)` they throw
`IllegalMonitorStateException`, and with it they never wake threads blocked in `await()`.
The intended methods are `await()`, `signal()` and `signalAll()`.

**How to fix.** Use the lock's own API.

```java
// Noncompliant
synchronized (lock) { balance += amount; }
notEmpty.notify();

// Compliant
lock.lock();
try { balance += amount; } finally { lock.unlock(); }
notEmpty.signal();
```

Only receivers the resolver proves to be one of these types are reported."#,
    ..RuleMeta::new(
        "java/intrinsic-lock-on-lock-object",
        "java",
        "Monitor operation on a java.util.concurrent lock",
    )
};

#[derive(Default)]
pub struct IntrinsicLockOnLockObject {
    locks: util::Gate,
}

const LOCK_TYPES: &[&str] = &[
    "java.util.concurrent.locks.Lock",
    "java.util.concurrent.locks.ReadWriteLock",
    "java.util.concurrent.locks.StampedLock",
    "java.util.concurrent.locks.Condition",
];

impl<'t> JavaRule<'t> for IntrinsicLockOnLockObject {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["synchronized_statement", "method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["wait", "notify", "notifyAll"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if kind(node) == "synchronized_statement" {
            if !self.locks.open(ctx, "locks") {
                return;
            }
            let Some(paren) = syntax::child_of_kind(node, "parenthesized_expression") else {
                return;
            };
            let lock = syntax::strip_parens(paren);
            if matches!(kind(lock), "this" | "class_literal" | "string_literal") {
                return;
            }
            let Some(t) = ctx.type_of(lock) else {
                return;
            };
            let Some(simple) = LOCK_TYPES
                .iter()
                .find(|f| ctx.is_subtype_of(&t, f))
                .map(|f| util::last_segment(f))
            else {
                return;
            };
            ctx.emit(
                lock,
                format!("Use `lock()`/`unlock()` instead: synchronizing on a `{simple}` uses its unrelated built-in monitor and does not acquire the lock"),
            );
            return;
        }
        let Some(name) = ctx.call_name(node) else {
            return;
        };
        if !matches!(name, "wait" | "notify" | "notifyAll") || !self.locks.open(ctx, "locks") {
            return;
        }
        if !util::receiver_is(ctx, node, LOCK_TYPES) {
            return;
        }
        let instead = match name {
            "wait" => "await()",
            "notify" => "signal()",
            _ => "signalAll()",
        };
        ctx.emit(
            node.field(Field::Name).unwrap_or(node),
            format!("Call `{instead}` (on a `Condition`): `{name}()` uses the object's built-in monitor, not the lock"),
        );
    }
}
