//! `java/clone-override` — implementing `clone()`.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["design"],
    remediation_minutes: 20,
    description: r#"Detects implementations of `clone()`, and classes that implement `Cloneable`
without declaring `clone()`.

**Why it matters.** The `Cloneable`/`Object.clone()` mechanism is fragile: it creates objects
without calling a constructor, copies fields shallowly (mutable state ends up shared between
copies), doesn't work with `final` fields that need a fresh value, forces casts and the checked
`CloneNotSupportedException`, and every subclass must remember to override it correctly.

A class that implements `Cloneable` but declares no `clone()` is a half-done version of the
same thing: `Object.clone()` stays `protected`, so no other class can copy it, and the marker
only suggests a copy facility that does not exist.

**How to fix.** Provide a copy constructor or a static copy factory that builds the new object
explicitly (and drop `Cloneable`).

```java
// Noncompliant
@Override
public Order clone() {
    try {
        return (Order) super.clone();
    } catch (CloneNotSupportedException e) {
        throw new AssertionError(e);
    }
}

// Compliant
public Order(Order other) {
    this.id = other.id;
    this.lines = new ArrayList<>(other.lines);
}
```

Not reported: abstract/interface declarations without a body, and `clone()` methods whose
only statement throws (the idiom that forbids cloning). A class implementing `Cloneable`
without `clone()` is not reported when it extends another class (which may provide the
`clone()` that `Cloneable` enables), when it calls `clone()` itself (`(Order) this.clone()` in a
copy method of its own), or in test code (fixtures that exercise `Cloneable` handling)."#,
    ..RuleMeta::new("java/clone-override", "java", "clone() implemented")
};

pub struct CloneOverride;

/// `class X implements Cloneable` (no superclass, not test code) that declares no `clone()` and
/// never calls it.
fn check_cloneable_class<'t>(node: Node<'t>, ctx: &Ctx<'t>) {
    if ctx.is_test() || node.field(Field::Superclass).is_some() {
        return;
    }
    let Some(interfaces) = node.field(Field::Interfaces) else {
        return;
    };
    let Some(cloneable) = named_children(interfaces)
        .flat_map(named_children)
        .find(|t| {
            util::type_simple_name(*t, ctx) == "Cloneable"
                && util::type_fqn(ctx, *t).as_deref() == Some("java.lang.Cloneable")
        })
    else {
        return;
    };
    let Some(body) = node.field(Field::Body) else {
        return;
    };
    let declares_clone = ctx.members(body).any(|m| {
        kind(m) == "method_declaration"
            && util::name_field(m).is_some_and(|n| ctx.text(n) == "clone")
            && syntax::parameters(m).next().is_none()
    });
    // `super.clone()`, `this.clone()`, `(T) clone()`: a copy method of its own relies on the marker.
    if declares_clone || ctx.text(body).contains("clone()") {
        return;
    }
    let name = node.field(Field::Name).map_or("", |n| ctx.text(n));
    ctx.emit(
        cloneable,
        format!(
            "`{name}` implements Cloneable but declares no clone(): nothing outside can copy \
             it; drop Cloneable and provide a copy constructor or factory"
        ),
    );
}

impl<'t> JavaRule<'t> for CloneOverride {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_declaration", "class_declaration"]
    }

    fn method_names(&self) -> &'static [&'static str] {
        &["clone"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if kind(node) == "class_declaration" {
            check_cloneable_class(node, ctx);
            return;
        }
        let Some(name) = util::name_field(node) else {
            return;
        };
        if ctx.text(name) != "clone" || syntax::parameters(node).next().is_some() {
            return;
        }
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        let mut stmts = util::statements(body);
        if let (Some(only), None) = (stmts.next(), stmts.next()) {
            if kind(only) == "throw_statement" {
                return;
            }
        }
        ctx.emit(
            name,
            "Replace clone() with a copy constructor or a copy factory method",
        );
    }
}
