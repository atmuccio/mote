//! `java/exception-not-thrown` — `new FooException(..);` as a statement, never thrown.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["error-handling"],
    remediation_minutes: 2,
    description: r#"Detects an exception object that is created as a statement of its own and then
dropped: `new IllegalArgumentException("bad id");` without `throw`.

**Why it matters.** Creating a `Throwable` does nothing by itself. The author almost certainly
meant to throw it, so the error condition the code was supposed to reject is silently
accepted and execution continues with invalid data.

**How to fix.** Add the missing `throw` (or remove the statement if it really is unneeded).

```java
// Noncompliant
if (id < 0) {
    new IllegalArgumentException("negative id: " + id);
}

// Compliant
if (id < 0) {
    throw new IllegalArgumentException("negative id: " + id);
}
```

The created type must be a proven `Throwable` subtype, or — for classes the resolver cannot
see (other files, libraries) — have a name ending in `Exception`. A name ending in `Error` is
not enough on its own, since plain classes are often named that way (error reports, error
response bodies, monitors that register themselves when constructed): such a type is reported
only when it is a JDK error (`java.lang.InternalError`, `java.io.IOError`, …) or a test-library
assertion error. Anonymous subclasses (`new Foo() { … };`) are not reported."#,
    ..RuleMeta::new(
        "java/exception-not-thrown",
        "java",
        "Exception created but not thrown",
    )
};

pub struct ExceptionNotThrown;

impl<'t> JavaRule<'t> for ExceptionNotThrown {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["object_creation_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if ctx
            .parent(node)
            .is_none_or(|p| kind(p) != "expression_statement")
        {
            return;
        }
        if syntax::children(node).any(|c| kind(c) == "class_body") {
            return;
        }
        let Some(ty) = util::fld(node, F::Type) else {
            return;
        };
        let base = util::type_base(ty);
        if !matches!(kind(base), "type_identifier" | "scoped_type_identifier") {
            return;
        }
        let text = ctx.text(base);
        let simple = text.rsplit('.').next().unwrap_or(text).trim();
        let throwable = match ctx
            .resolve_type(ty)
            .and_then(|t| ctx.subtype(&t, "java.lang.Throwable"))
        {
            Some(b) => b,
            None => {
                looks_like_exception(simple)
                    || (looks_like_error(simple) && is_known_error_type(ctx, text.trim(), base))
            }
        };
        if !throwable {
            return;
        }
        ctx.emit(
            ty,
            format!("Throw this `{simple}` or remove it; creating an exception without throwing it has no effect"),
        );
    }
}

/// Name heuristic for exception classes the resolver cannot see.
fn looks_like_exception(simple: &str) -> bool {
    (simple.ends_with("Exception") && simple.len() > "Exception".len())
        || simple == "Exception"
        || simple == "Throwable"
}

fn looks_like_error(simple: &str) -> bool {
    simple.ends_with("Error") && simple.len() > "Error".len()
}

/// `java.lang` errors missing from the resolver's JDK table (the table has `AssertionError`,
/// `LinkageError`, `OutOfMemoryError`, …, which resolve as `Throwable` directly).
const JAVA_LANG_ERRORS: &[&str] = &[
    "AbstractMethodError",
    "BootstrapMethodError",
    "ClassCircularityError",
    "ClassFormatError",
    "IllegalAccessError",
    "IncompatibleClassChangeError",
    "InstantiationError",
    "InternalError",
    "NoSuchFieldError",
    "NoSuchMethodError",
    "UnknownError",
    "UnsatisfiedLinkError",
    "UnsupportedClassVersionError",
    "VerifyError",
];

/// Assertion errors of test libraries (they extend `AssertionError`).
const LIBRARY_ERRORS: &[&str] = &[
    "junit.framework.AssertionFailedError",
    "org.opentest4j.AssertionFailedError",
    "org.opentest4j.MultipleFailuresError",
];

/// Whether an `…Error` type the resolver cannot prove `Throwable` still certainly is one: a
/// JDK error (imported or written qualified from a `java.` / `javax.xml.` package, or a
/// `java.lang` error used by its simple name) or a test-library assertion error. Other `…Error`
/// classes (of this project or a library) are often not exceptions at all.
fn is_known_error_type<'t>(ctx: &Ctx<'t>, name: &str, at: Node<'t>) -> bool {
    match ctx.resolve_type_name(name, at) {
        Some(fqn) => {
            fqn.starts_with("java.")
                || fqn.starts_with("javax.xml.")
                || LIBRARY_ERRORS.contains(&fqn.as_str())
        }
        None => JAVA_LANG_ERRORS.contains(&name),
    }
}
