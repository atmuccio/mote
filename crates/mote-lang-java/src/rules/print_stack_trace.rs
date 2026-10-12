//! `java/print-stack-trace` — a resolver-based rule using `declaration_of` and `type_of`.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Medium,
    tags: &["logging", "error-handling"],
    remediation_minutes: 5,
    description: r#"Detects `e.printStackTrace()` (without arguments) on exceptions.

**Why it matters.** `printStackTrace()` writes to the process's standard error stream: the
output bypasses the logging configuration (levels, formats, destinations, correlation ids), is
often lost entirely in containers and application servers, and can leak internal details to
users of console applications.

**How to fix.** Log the exception through the application's logger, passing the exception
itself so the stack trace is kept — or rethrow it.

```java
// Noncompliant
try { sync(); } catch (IOException e) { e.printStackTrace(); }

// Compliant
try { sync(); } catch (IOException e) { log.error("sync failed", e); }
```

Test code is not checked. The receiver must be a caught exception or have a known `Throwable`
type."#,
    ..RuleMeta::new(
        "java/print-stack-trace",
        "java",
        "printStackTrace() instead of logging",
    )
};

pub struct PrintStackTrace;

impl<'t> JavaRule<'t> for PrintStackTrace {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["printStackTrace"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if ctx.is_test() {
            return;
        }
        let Some(name) = node.field(Field::Name) else {
            return;
        };
        if ctx.text(name) != "printStackTrace" || syntax::argument_count(node) != 0 {
            return;
        }
        let Some(recv) = node.field(Field::Object).map(syntax::strip_parens) else {
            return;
        };
        let caught = kind(recv) == "identifier"
            && ctx
                .declaration_of(recv)
                .is_some_and(|d| d.kind == VarKind::CatchParameter);
        let throwable = caught
            || ctx
                .type_of(recv)
                .is_some_and(|t| ctx.is_subtype_of(&t, "java.lang.Throwable"));
        if throwable {
            ctx.emit(name, "Log this exception with the application logger instead of printing its stack trace");
        }
    }
}
