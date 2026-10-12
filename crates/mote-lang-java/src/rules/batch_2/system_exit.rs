//! `java/system-exit` — `System.exit` / `Runtime.exit` / `Runtime.halt` outside `main`.

use super::util::{field, Mods, FIELDS};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["design", "error-handling"],
    remediation_minutes: 15,
    description: r#"Detects calls to `System.exit(…)`, `Runtime.getRuntime().exit(…)`, and
`Runtime.getRuntime().halt(…)` outside a `main` method.

**Why it matters.** Terminating the JVM from library or application code takes the decision
away from the caller: it kills application servers, build tools and test runners that host
the code, skips `finally` blocks of the calling threads (and, with `halt`, shutdown hooks),
and makes the code impossible to unit test.

**How to fix.** Throw an exception or return a status, and let the program's entry point
(`main`) decide whether to exit.

```java
// Noncompliant
void loadConfig(Path p) {
    if (!Files.exists(p)) {
        System.exit(2);
    }
}

// Compliant
void loadConfig(Path p) {
    if (!Files.exists(p)) {
        throw new ConfigException("missing " + p);
    }
}
```

Calls anywhere inside a `static main` method (including lambdas and anonymous classes there)
are not reported."#,
    ..RuleMeta::new("java/system-exit", "java", "JVM terminated outside main")
};

pub struct SystemExit;

impl<'t> JavaRule<'t> for SystemExit {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["exit", "halt"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) = field(node, FIELDS.name) else {
            return;
        };
        let method = ctx.text(name);
        if !matches!(method, "exit" | "halt") {
            return;
        }
        let what = if method == "exit" && ctx.is_call_to(node, "java.lang.System", "exit") {
            "System.exit"
        } else if ctx.is_call_to(node, "java.lang.Runtime", method) {
            if method == "exit" {
                "Runtime.exit"
            } else {
                "Runtime.halt"
            }
        } else {
            return;
        };
        let in_main = ctx.ancestors(node).any(|a| {
            kind(a) == "method_declaration"
                && syntax::name_of(a, ctx.source()) == "main"
                && Mods::of(a).has("static")
        });
        if in_main {
            return;
        }
        let start = field(node, FIELDS.object).unwrap_or(name);
        ctx.emit_range(
            Span::cover(start, name),
            format!("Throw an exception or return a status instead of calling {what} here"),
        );
    }
}
