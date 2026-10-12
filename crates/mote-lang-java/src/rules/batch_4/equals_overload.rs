//! `java/equals-overload` — `boolean equals(Foo other)` overloads instead of overriding.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["equality"],
    remediation_minutes: 10,
    description: r#"Detects an instance method `boolean equals(X)` whose parameter type is not
`Object`.

**Why it matters.** Such a method *overloads* `Object.equals(Object)` instead of overriding
it. Collections (`HashSet`, `HashMap`, `List.contains`), `Objects.equals`, assertion
libraries, and any caller holding the object as `Object` still use identity comparison, so
two "equal" values are treated as different — while direct calls with the static type `X`
behave differently. The result depends on the compile-time type of the argument.

**How to fix.** Declare `equals(Object)`, check the type inside, and add `@Override` so the
compiler verifies the signature.

```java
// Noncompliant
public boolean equals(Point other) {
    return x == other.x && y == other.y;
}

// Compliant
@Override
public boolean equals(Object o) {
    return o instanceof Point other && x == other.x && y == other.y;
}
```

Not reported: static methods, methods annotated `@Override` (they implement some interface
method), and classes that also declare `equals(Object)` (a deliberate typed helper next to
the real override)."#,
    ..RuleMeta::new(
        "java/equals-overload",
        "java",
        "equals() overloaded instead of overridden",
    )
};

pub struct EqualsOverload;

/// Whether a declaration's single parameter type is `Object`.
fn param_is_object<'t>(ctx: &Ctx<'t>, method: Node<'t>) -> Option<bool> {
    let mut params = syntax::parameters(method);
    let (Some(p), None) = (params.next(), params.next()) else {
        return None;
    };
    if kind(p) != "formal_parameter" {
        return None;
    }
    let ty = util::fld(p, F::Type)?;
    Some(matches!(ctx.text(ty), "Object" | "java.lang.Object"))
}

impl<'t> JavaRule<'t> for EqualsOverload {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_declaration"]
    }

    fn method_names(&self) -> &'static [&'static str] {
        &["equals"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) = util::fld(node, F::Name) else {
            return;
        };
        if ctx.text(name) != "equals" {
            return;
        }
        if util::fld(node, F::Type).is_none_or(|t| kind(t) != "boolean_type") {
            return;
        }
        if param_is_object(ctx, node) != Some(false) {
            return;
        }
        if syntax::has_modifier(node, "static")
            || syntax::has_annotation(node, ctx.source(), "Override")
        {
            return;
        }
        // A class that also overrides equals(Object) uses the overload as a typed helper.
        let Some(body) = ctx.parent(node) else {
            return;
        };
        let also_overrides = ctx.members(body).any(|m| {
            kind(m) == "method_declaration"
                && syntax::name_of(m, ctx.source()) == "equals"
                && param_is_object(ctx, m) == Some(true)
        });
        if also_overrides {
            return;
        }
        let param_type = syntax::parameters(node)
            .next()
            .and_then(|p| util::fld(p, F::Type))
            .map(|t| ctx.text(t))
            .unwrap_or("?");
        ctx.emit(
            name,
            format!("Declare `equals(Object)` instead: `equals({param_type})` overloads it, so collections and callers holding an Object never call this method"),
        );
    }
}
