//! `java/application-error-subclass` — an application class extending `java.lang.Error`
//! directly.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["error-handling", "exceptions"],
    remediation_minutes: 5,
    description: r#"Detects classes declared with `extends Error` (`java.lang.Error`).

**Why it matters.** `Error` is the branch of the throwable hierarchy for failures of the
virtual machine itself — out of memory, stack overflow, a class that fails to link — which an
application is not expected to handle. Handlers are written accordingly: `catch (Exception e)`
and most frameworks' error boundaries let an `Error` pass, logging and cleanup code often
skips it, and some runtimes stop the thread or the process on one. An application failure
thrown as an `Error` therefore escapes the handling written for application failures.

**How to fix.** Extend `RuntimeException` for failures callers are not expected to handle,
or `Exception` for ones they must declare or handle.

```java
// Noncompliant
public class ConfigurationError extends Error {
    public ConfigurationError(String message) { super(message); }
}

// Compliant
public class ConfigurationException extends RuntimeException {
    public ConfigurationException(String message) { super(message); }
}
```

Only direct subclasses of `java.lang.Error` are reported: subclasses of `AssertionError`
(custom assertion failures of test libraries) and of the JDK's other `Error` types, and
classes extending an `Error` class declared in this file or imported from elsewhere, are left
alone."#,
    ..RuleMeta::new(
        "java/application-error-subclass",
        "java",
        "Application class extending java.lang.Error",
    )
};

pub struct ApplicationErrorSubclass;

impl<'t> JavaRule<'t> for ApplicationErrorSubclass {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["class_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(sup) = node.field(Field::Superclass) else {
            return;
        };
        let Some(t) = syntax::first_named_child(sup) else {
            return;
        };
        let text = ctx.text(t);
        if text != "Error" && text != "java.lang.Error" {
            return;
        }
        if ctx.resolve_type_name(text, node).as_deref() != Some("java.lang.Error") {
            return;
        }
        let Some(name) = node.field(Field::Name) else {
            return;
        };
        ctx.emit(
            name,
            format!(
                "Make `{}` extend RuntimeException or Exception: `Error` is meant for failures of the JVM itself",
                ctx.text(name)
            ),
        );
    }
}
