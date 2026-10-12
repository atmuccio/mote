//! `java/system-out-print` — printing to `System.out` / `System.err` in production code.

use super::util::{field, in_junit3_test_class, Mods, FIELDS};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Medium,
    tags: &["logging"],
    remediation_minutes: 5,
    description: r#"Detects `print`, `println`, `printf`, `format`, `write`, and `append` calls on
`System.out` or `System.err` outside test files.

**Why it matters.** Console output bypasses the logging configuration: it has no level,
timestamp, or source, cannot be filtered or routed, is often lost in containers and
application servers, and is written synchronously, which can slow down hot paths.

**How to fix.** Use the application's logger. Command-line tools that are meant to talk to a
terminal can suppress the rule where console output is the intended user interface.

```java
// Noncompliant
System.out.println("Loaded " + n + " records");

// Compliant
log.info("Loaded {} records", n);
```

Not reported: test code — test files, JUnit 3 test classes kept under a main source root (a
class extending `TestCase` or a `…TestCase` base class) and `@Test` methods — and files
declaring a `static main` method — command-line programs, whose console output is their user
interface. `System` must resolve to `java.lang.System` (a
class of the same name declared in the file is not confused with it)."#,
    ..RuleMeta::new(
        "java/system-out-print",
        "java",
        "Printing to System.out or System.err",
    )
};

#[derive(Default)]
pub struct SystemOutPrint {
    /// Console writes found so far: (location, `out` or `err`).
    pending: Vec<(Span, bool)>,
    /// The file declares a `static main` method (a command-line program).
    has_main: bool,
}

impl<'t> JavaRule<'t> for SystemOutPrint {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation", "method_declaration"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["print", "println", "printf", "format", "write", "append"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if ctx.is_test() || self.has_main {
            return;
        }
        if kind(node) == "method_declaration" {
            self.has_main = ctx.name_of(node) == "main" && Mods::of(node).has("static");
            return;
        }
        let Some(name) = field(node, FIELDS.name) else {
            return;
        };
        if !matches!(
            ctx.text(name),
            "print" | "println" | "printf" | "format" | "write" | "append"
        ) {
            return;
        }
        let Some(obj) = field(node, FIELDS.object) else {
            return;
        };
        if kind(obj) != "field_access" {
            return;
        }
        let (Some(system), Some(stream)) = (field(obj, FIELDS.object), field(obj, FIELDS.field))
        else {
            return;
        };
        let stream_name = ctx.text(stream);
        if !matches!(stream_name, "out" | "err")
            || !matches!(ctx.text(system), "System" | "java.lang.System")
        {
            return;
        }
        if ctx.type_name_of(system).as_deref() != Some("java.lang.System") {
            return;
        }
        let in_test_method = ctx
            .enclosing_method(node)
            .is_some_and(|m| syntax::has_annotation(m, ctx.source(), "Test"));
        if in_test_method || in_junit3_test_class(ctx, node) {
            return;
        }
        self.pending
            .push((Span::cover(obj, name), stream_name == "err"));
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        if self.has_main {
            return;
        }
        for (span, err) in &self.pending {
            let stream = if *err { "err" } else { "out" };
            ctx.emit_range(
                *span,
                format!("Use a logger instead of writing to System.{stream}"),
            );
        }
    }
}
