//! `java/generic-exception-thrown` — `throw new RuntimeException(…)` and friends.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["error-handling"],
    remediation_minutes: 10,
    description: r#"Detects `throw new RuntimeException(…)`, `throw new Exception(…)`,
`throw new Throwable(…)`, and `throw new Error(…)`.

**Why it matters.** Callers can only handle a generic exception by catching the generic type,
which also catches every unrelated failure. The type carries no meaning, so the only way to
tell failures apart is parsing messages.

**How to fix.** Throw a specific standard exception (`IllegalArgumentException`,
`IllegalStateException`, `UncheckedIOException`, …) or a dedicated domain exception.

```java
// Noncompliant
if (port < 0) {
    throw new RuntimeException("negative port " + port);
}

// Compliant
if (port < 0) {
    throw new IllegalArgumentException("negative port " + port);
}
```

Test code is not checked (tests throw generic exceptions to simulate failures)."#,
    ..RuleMeta::new(
        "java/generic-exception-thrown",
        "java",
        "Generic exception thrown",
    )
};

pub struct GenericExceptionThrown;

impl<'t> JavaRule<'t> for GenericExceptionThrown {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["throw_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if ctx.is_test() {
            return;
        }
        let Some(thrown) = syntax::first_named_child(node).map(syntax::strip_parens) else {
            return;
        };
        if kind(thrown) != "object_creation_expression"
            || syntax::child_of_kind(thrown, "class_body").is_some()
        {
            return;
        }
        let Some(t) = thrown.field(Field::Type) else {
            return;
        };
        let simple = util::type_simple_name(t, ctx);
        if !matches!(
            simple,
            "RuntimeException" | "Exception" | "Throwable" | "Error"
        ) {
            return;
        }
        let Some(fqn) = util::type_fqn(ctx, t) else {
            return;
        };
        if matches!(
            fqn.as_str(),
            "java.lang.RuntimeException"
                | "java.lang.Exception"
                | "java.lang.Throwable"
                | "java.lang.Error"
        ) {
            ctx.emit(
                t,
                format!("Throw a more specific exception type than `{simple}`"),
            );
        }
    }
}
