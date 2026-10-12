//! `java/optional-return-null` — `return null;` from a method returning `Optional`.

use super::util::{Fld, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["optional", "null-safety"],
    remediation_minutes: 2,
    description: r#"Detects `return null;` in a method whose declared return type is `Optional<…>`,
`OptionalInt`, `OptionalLong`, or `OptionalDouble` (the JDK types, or Guava's `Optional`).

**Why it matters.** Callers of a method returning `Optional` rely on never getting `null` —
that is the whole contract of the type. They call `isPresent()`, `map(..)`, or `orElse(..)`
directly on the result and fail with a `NullPointerException` instead of handling the "no
value" case.

**How to fix.** Return an empty optional: `Optional.empty()` (`OptionalInt.empty()`, …, or
Guava's `Optional.absent()`).

```java
// Noncompliant
Optional<User> find(String id) {
    if (id == null) {
        return null;
    }
    return Optional.ofNullable(users.get(id));
}

// Compliant
Optional<User> find(String id) {
    if (id == null) {
        return Optional.empty();
    }
    return Optional.ofNullable(users.get(id));
}
```

`return null` inside a lambda or an anonymous class within such a method is not reported (it
returns from that inner function, whose type is not the method's)."#,
    ..RuleMeta::new(
        "java/optional-return-null",
        "java",
        "null returned from a method returning Optional",
    )
};

pub struct OptionalReturnNull;

const OPTIONALS: &[&str] = &[
    "java.util.Optional",
    "java.util.OptionalInt",
    "java.util.OptionalLong",
    "java.util.OptionalDouble",
    "com.google.common.base.Optional",
];

impl<'t> JavaRule<'t> for OptionalReturnNull {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["return_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(value) = syntax::first_named_child(node) else {
            return;
        };
        if !syntax::is_null_literal(value) {
            return;
        }
        let Some(func) = ctx.enclosing_function(node) else {
            return;
        };
        if kind(func) != "method_declaration" {
            return;
        }
        let Some(ty) = func.fld(F::Type) else {
            return;
        };
        // `Optional<T>` → `Optional`; `java.util.Optional<T>` → `java.util.Optional`.
        let base = if kind(ty) == "generic_type" {
            match named_children(ty).find(|c| kind(*c) != "type_arguments") {
                Some(b) => b,
                None => return,
            }
        } else {
            ty
        };
        let name = ctx.text(base);
        if !name.ends_with("Optional")
            && !name.ends_with("OptionalInt")
            && !name.ends_with("OptionalLong")
            && !name.ends_with("OptionalDouble")
        {
            return;
        }
        let Some(fqn) = ctx.resolve_type_name(name, base) else {
            return;
        };
        if !OPTIONALS.contains(&fqn.as_str()) {
            return;
        }
        let simple = fqn.rsplit('.').next().unwrap_or("Optional");
        let replacement = if fqn.starts_with("com.google") {
            "Optional.absent()".to_string()
        } else {
            format!("{simple}.empty()")
        };
        ctx.emit(
            syntax::strip_parens(value),
            format!(
                "Return {replacement} instead of null from a method declared to return {simple}"
            ),
        );
    }
}
