//! `java/wait-notify-outside-synchronized` — `wait()`/`notify()` without holding the monitor.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["concurrency", "multi-threading"],
    remediation_minutes: 10,
    description: r#"Detects `wait()`, `notify()`, and `notifyAll()` called on the current object
(unqualified or through `this`) outside any `synchronized` block or `synchronized` method.

**Why it matters.** These methods require the caller to own the object's monitor; otherwise
they throw `IllegalMonitorStateException` at run time. Even when a caller happens to hold the
lock, a condition wait that isn't lexically guarded is easy to break during refactoring and
prone to lost wake-ups.

**How to fix.** Call them inside `synchronized (this) { … }` or from a `synchronized` method,
and wait in a loop that re-checks the condition — or use `java.util.concurrent` tools
(`CountDownLatch`, `BlockingQueue`, `Condition`).

```java
// Noncompliant
void awaitReady() throws InterruptedException {
    while (!ready) {
        wait();
    }
}

// Compliant
synchronized void awaitReady() throws InterruptedException {
    while (!ready) {
        wait();
    }
}
```

Calls inside lambdas (whose calling context is unknown) and classes that declare their own
`wait`/`notify` overloads are not reported."#,
    ..RuleMeta::new(
        "java/wait-notify-outside-synchronized",
        "java",
        "wait/notify outside synchronized code",
    )
};

pub struct WaitNotifyOutsideSynchronized;

impl<'t> JavaRule<'t> for WaitNotifyOutsideSynchronized {
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
        let Some(name) = util::name_field(node) else {
            return;
        };
        let method = ctx.text(name);
        let max_args = match method {
            "wait" => 2,
            "notify" | "notifyAll" => 0,
            _ => return,
        };
        if util::arg_count(node) > max_args {
            return;
        }
        if let Some(obj) = util::object_field(node) {
            if kind(obj) != "this" {
                return;
            }
        }
        for a in ctx.ancestors(node) {
            match kind(a) {
                "synchronized_statement" | "lambda_expression" => return,
                "method_declaration" => {
                    if syntax::has_modifier(a, "synchronized") {
                        return;
                    }
                    break;
                }
                "constructor_declaration"
                | "compact_constructor_declaration"
                | "static_initializer" => break,
                "class_body" | "interface_body" | "enum_body" | "enum_body_declarations" => return,
                _ => {}
            }
        }
        // A class may declare its own overloads (`wait(String)`, `notify(Event)`).
        let own = ctx
            .enclosing_class(node)
            .and_then(syntax::type_body)
            .is_some_and(|body| util::declares_method(ctx, body, method));
        if own {
            return;
        }
        ctx.emit(
            name,
            format!("Call `{method}()` only while holding this object's monitor, inside a synchronized block or method"),
        );
    }
}
