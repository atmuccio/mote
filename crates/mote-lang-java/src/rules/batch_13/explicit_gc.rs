//! `java/explicit-gc` — `System.gc()` / `Runtime.gc()` in production code.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["performance", "gc"],
    remediation_minutes: 5,
    description: r#"Detects explicit requests for a garbage collection: `System.gc()` and
`Runtime.getRuntime().gc()` (or `gc()` on any `Runtime`) outside test code.

**Why it matters.** With the default collectors the call triggers a full, stop-the-world
collection of the whole heap: every application thread pauses, for seconds on large heaps,
at a moment the collector did not choose. It rarely fixes the problem it was added for (memory
that is still referenced is not freed; memory that is not referenced would have been freed
anyway), and its effect depends on JVM flags (`-XX:+DisableExplicitGC` turns it into a no-op,
`-XX:+ExplicitGCInvokesConcurrent` changes it), so code that relies on it behaves differently
in each deployment.

**How to fix.** Remove the call and fix the underlying retention (clear references, close
resources, bound caches). Tune the collector with JVM options if pauses are the concern.

```java
// Noncompliant
cache.clear();
System.gc();

// Compliant
cache.clear();
```

Not reported: test code, where forcing a collection to observe weak or phantom references
is a legitimate technique, and benchmark code — files that use JMH (`org.openjdk.jmh`
imports) or sit in a `jmh` source set (`src/jmh/java`) — where collecting before measuring
memory or time is part of the method."#,
    ..RuleMeta::new(
        "java/explicit-gc",
        "java",
        "Explicit garbage collection request",
    )
};

pub struct ExplicitGc;

impl<'t> JavaRule<'t> for ExplicitGc {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["gc"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if ctx.call_name(node) != Some("gc") || syntax::argument_count(node) != 0 || ctx.is_test() {
            return;
        }
        if is_benchmark(ctx) {
            return;
        }
        let owner = if ctx.is_call_to(node, "java.lang.System", "gc") {
            "System"
        } else if ctx.is_call_to(node, "java.lang.Runtime", "gc") {
            "Runtime"
        } else {
            return;
        };
        let Some(name) = node.field(Field::Name) else {
            return;
        };
        ctx.emit(
            name,
            format!("Remove this `{owner}.gc()` call: it forces a full collection that pauses the application"),
        );
    }
}

/// Benchmark harness code: a file using JMH (`org.openjdk.jmh…` imports) or in a `jmh` source
/// set (`src/jmh/java`). Forcing a collection before a measurement is part of the method there.
fn is_benchmark(ctx: &Ctx<'_>) -> bool {
    let path = ctx.path();
    path.starts_with("src/jmh/")
        || path.contains("/src/jmh/")
        || ctx
            .imports()
            .iter()
            .any(|i| i.path == "org.openjdk.jmh" || i.path.starts_with("org.openjdk.jmh."))
}
