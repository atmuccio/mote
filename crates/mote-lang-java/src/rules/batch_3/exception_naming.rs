//! `java/exception-naming` — exception classes must be named `…Exception` / `…Error`.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["naming", "error-handling"],
    remediation_minutes: 5,
    description: r#"Detects classes that extend an exception type but whose name doesn't end
in `Exception` or `Error`.

**Why it matters.** The suffix is how Java developers recognise throwables at a glance: in
`throws` clauses, catch blocks, imports, and stack traces. A class called `InvalidInput` or
`Timeout` reads like a value, not something that is thrown.

**How to fix.** Rename the class so its name ends in `Exception` (or `Error` for subclasses of
`java.lang.Error`).

```java
// Noncompliant
public class InvalidOrder extends RuntimeException { … }

// Compliant
public class InvalidOrderException extends RuntimeException { … }
```

The superclass is recognised by name (`…Exception`, `Throwable`) or, for names ending in
`Error`, only when the resolver proves it is a `Throwable` (many `…Error` classes are plain
data types)."#,
    ..RuleMeta::new(
        "java/exception-naming",
        "java",
        "Exception class name without Exception suffix",
    )
};

pub struct ExceptionNaming;

impl<'t> JavaRule<'t> for ExceptionNaming {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["class_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) = util::name_field(node) else {
            return;
        };
        let class_name = ctx.text(name);
        if class_name.ends_with("Exception") || class_name.ends_with("Error") {
            return;
        }
        let Some(sup) = node
            .field(Field::Superclass)
            .and_then(syntax::first_named_child)
        else {
            return;
        };
        let sup_name = util::type_simple_name(sup, ctx);
        let by_name = sup_name.ends_with("Exception") || sup_name == "Throwable";
        let sup_type = ctx.resolve_type(sup);
        let proven = sup_type
            .as_ref()
            .and_then(|t| ctx.subtype(t, "java.lang.Throwable"));
        let throwable = match proven {
            Some(p) => p,
            // Unknown hierarchy: trust the conventional `…Exception` / `Throwable` names only.
            None => by_name,
        };
        if !throwable {
            return;
        }
        let is_error = sup_type
            .as_ref()
            .is_some_and(|t| ctx.is_subtype_of(t, "java.lang.Error"));
        let suffix = if is_error { "Error" } else { "Exception" };
        ctx.emit(
            name,
            format!("Rename `{class_name}` so that it ends with `{suffix}`"),
        );
    }
}
