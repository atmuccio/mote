//! `java/sleep-in-test` — `Thread.sleep` in test code.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["tests", "flaky"],
    remediation_minutes: 15,
    description: r#"Detects `Thread.sleep(…)` (and `TimeUnit.X.sleep(…)`) in test code.

**Why it matters.** A fixed sleep is a guess about how long something takes. Too short and the
test fails randomly on a slow or busy CI machine; too long and the whole suite gets slower with
every such wait. Either way the test doesn't state what it is actually waiting for.

**How to fix.** Wait for the condition itself: join the thread, wait on a `CountDownLatch` or
`Future` with a timeout, poll with a deadline (for example with Awaitility), or inject a
controllable clock so no real time has to pass.

```java
// Noncompliant
service.startAsync();
Thread.sleep(2000);
assertTrue(service.isRunning());

// Compliant
service.startAsync();
service.awaitRunning(5, TimeUnit.SECONDS);
assertTrue(service.isRunning());
```

Only test files are checked."#,
    ..RuleMeta::new("java/sleep-in-test", "java", "Thread.sleep in a test")
};

pub struct SleepInTest;

impl<'t> JavaRule<'t> for SleepInTest {
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
        if !ctx.is_test() {
            return;
        }
        let Some(name) = util::name_field(node) else {
            return;
        };
        if ctx.text(name) != "sleep" {
            return;
        }
        if ctx.is_call_to(node, "java.lang.Thread", "sleep")
            || ctx.is_call_to(node, "java.util.concurrent.TimeUnit", "sleep")
        {
            ctx.emit(
                name,
                "Wait for the expected condition (latch, future, polling with a timeout) instead of sleeping in a test",
            );
        }
    }
}
