//! `java/class-name-string-comparison` — `getClass().getSimpleName().equals("Foo")`.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["types", "refactoring"],
    remediation_minutes: 5,
    description: r#"Detects a runtime class name compared with a string literal through `equals`:
`x.getClass().getSimpleName().equals("Foo")`, and `x.getClass().getName()` (or
`getCanonicalName()` / `getTypeName()`) compared with a name that has no package.

**Why it matters.** The compiler cannot check a class name hidden in a string: renaming the
class silently breaks the comparison, simple names collide across packages, and subclasses
or proxies (`Foo$$EnhancerByCGLIB`) never match. Worse, `getName()` returns the *fully
qualified* name, so comparing it with `"SQLTimeoutException"` is always `false`.

**How to fix.** Compare `Class` objects or use `instanceof`.

```java
// Noncompliant
if (event.getClass().getSimpleName().equals("OrderCreated")) { … }
if (ex.getClass().getName().equals("SQLTimeoutException")) { … }   // always false

// Compliant
if (event instanceof OrderCreated created) { … }
if (ex instanceof SQLTimeoutException) { … }
```

Not reported: `getName()` compared with a fully qualified literal (a common way to test for
an optional class without depending on it), prefix tests such as `startsWith("java.")`, and
non-literal values."#,
    ..RuleMeta::new(
        "java/class-name-string-comparison",
        "java",
        "Class compared by name",
    )
};

#[derive(Default)]
pub struct ClassNameStringComparison {
    get_class: util::Gate,
}

/// For `x.getClass().getName()` (or another name getter) with no arguments, the getter name.
fn class_name_getter<'t>(ctx: &Ctx<'t>, expr: Node<'t>) -> Option<&'t str> {
    let expr = syntax::strip_parens(expr);
    if kind(expr) != "method_invocation" || util::arg_count(expr) != 0 {
        return None;
    }
    let getter = util::name_text(ctx, expr);
    if !matches!(
        getter,
        "getName" | "getSimpleName" | "getCanonicalName" | "getTypeName"
    ) {
        return None;
    }
    let inner = util::fld(expr, F::Object).map(syntax::strip_parens)?;
    (kind(inner) == "method_invocation"
        && util::name_text(ctx, inner) == "getClass"
        && util::arg_count(inner) == 0)
        .then_some(getter)
}

impl<'t> JavaRule<'t> for ClassNameStringComparison {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["equals"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !self.get_class.open(ctx, "getClass()") {
            return;
        }
        let Some(name) = util::fld(node, F::Name) else {
            return;
        };
        if ctx.text(name) != "equals" || util::arg_count(node) != 1 {
            return;
        }
        let (Some(recv), Some(arg)) = (util::fld(node, F::Object), util::args(node).next()) else {
            return;
        };
        let (literal, getter) = if syntax::is_string_literal(arg) {
            (arg, class_name_getter(ctx, recv))
        } else if syntax::is_string_literal(recv) {
            (recv, class_name_getter(ctx, arg))
        } else {
            return;
        };
        let Some(getter) = getter else {
            return;
        };
        let Some(value) = util::string_value(ctx, literal) else {
            return;
        };
        if getter == "getSimpleName" {
            ctx.emit(
                name,
                "Compare Class objects or use instanceof instead of comparing the simple class name with a string",
            );
        } else if !value.contains('.') && !value.is_empty() {
            ctx.emit(
                name,
                format!("This is always false: `{getter}()` returns the fully qualified name, never `{value}`; compare Class objects or use instanceof"),
            );
        }
    }
}
