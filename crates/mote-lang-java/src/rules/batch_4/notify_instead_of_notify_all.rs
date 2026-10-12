//! `java/notify-instead-of-notify-all` — `notify()` wakes an arbitrary single waiter.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["concurrency", "multi-threading"],
    remediation_minutes: 5,
    description: r#"Detects calls to `notify()`.

**Why it matters.** `notify()` wakes exactly one thread, chosen arbitrarily, among those
waiting on the monitor. When threads wait for *different* conditions on the same object (a
bounded buffer's "not full" and "not empty"), the wrong one can be woken, go back to
waiting, and leave the thread that could make progress asleep — a lost wake-up that shows
up as a rare hang. `notifyAll()` lets every waiter re-check its own condition.

**How to fix.** Use `notifyAll()`, or move to `java.util.concurrent` (`Condition.signal`
with one condition per predicate, `BlockingQueue`, `CountDownLatch`, …).

```java
// Noncompliant
synchronized void put(Item item) {
    queue.add(item);
    notify();
}

// Compliant
synchronized void put(Item item) {
    queue.add(item);
    notifyAll();
}
```

`notify()` is correct when every waiter waits for the same condition and any one of them
can handle it; this rule is informational in that case."#,
    ..RuleMeta::new(
        "java/notify-instead-of-notify-all",
        "java",
        "notify() instead of notifyAll()",
    )
};

pub struct NotifyInsteadOfNotifyAll;

impl<'t> JavaRule<'t> for NotifyInsteadOfNotifyAll {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["notify"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        // O(1) text test before touching the tree.
        if !ctx.text(node).ends_with("notify()") {
            return;
        }
        let Some(name) = util::fld(node, F::Name) else {
            return;
        };
        if ctx.text(name) != "notify" || util::arg_count(node) != 0 {
            return;
        }
        // `Object.notify()` is final, so every no-argument `notify()` is the monitor method;
        // only `super.notify()` in weird code is left alone.
        if util::fld(node, F::Object).is_some_and(|o| kind(o) == "super") {
            return;
        }
        ctx.emit(
            name,
            "Use `notifyAll()`: `notify()` wakes one arbitrary waiter, which may not be the one that can proceed",
        );
    }
}
