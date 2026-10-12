//! `java/object-method-name-typo` — `hashcode()`, `tostring()`, `Equals(Object)`,
//! `equal(Object)`: near-misses of the `Object` methods that override nothing.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["typo", "equality", "overrides"],
    remediation_minutes: 2,
    description: r#"Detects instance methods whose name and parameters almost match an `Object`
method: `hashcode()` / `HashCode()`, `tostring()` / `ToString()`, `Equals(Object)`, and
`equal(Object)` returning `boolean`; and a `finalize(..)` with parameters that is written as a
finalizer (it declares `throws Throwable` or calls `super.finalize()`), which the garbage
collector never calls.

**Why it matters.** The method was meant to override `hashCode`, `toString` or `equals`, but
Java names are case-sensitive, so it is a new, unrelated method that nothing calls. The class
keeps the identity-based `Object` behavior: equal objects get different hash codes, hash-based
collections lose entries, and logs print `Order@1b6d3586`.

**How to fix.** Use the exact name and add `@Override`, which makes the compiler catch the
next typo.

```java
// Noncompliant
public int hashcode() { return Objects.hash(id); }

// Compliant
@Override
public int hashCode() { return Objects.hash(id); }
```

Static methods and classes that also declare the correctly spelled method are not
reported."#,
    ..RuleMeta::new(
        "java/object-method-name-typo",
        "java",
        "Misspelled Object method",
    )
};

pub struct ObjectMethodNameTypo;

impl<'t> JavaRule<'t> for ObjectMethodNameTypo {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name_node) = ctx.name_node(node) else {
            return;
        };
        let name = ctx.text(name_node);
        if name == "finalize" {
            check_finalizer_with_parameters(node, name_node, ctx);
            return;
        }
        let (canonical, params, ret) = if name.eq_ignore_ascii_case("hashCode") {
            ("hashCode", 0, "int")
        } else if name.eq_ignore_ascii_case("toString") {
            ("toString", 0, "String")
        } else if name.eq_ignore_ascii_case("equals") || name == "equal" {
            ("equals", 1, "boolean")
        } else {
            return;
        };
        if name == canonical {
            return;
        }
        if node.field(Field::Type).map(|t| ctx.text(t)) != Some(ret) {
            return;
        }
        let mut ps = syntax::parameters(node);
        let first = ps.next();
        if ps.next().is_some() || first.is_some() != (params == 1) {
            return;
        }
        if let Some(p) = first {
            let ty = p.field(Field::Type).map(|t| ctx.text(t));
            if !matches!(ty, Some("Object" | "java.lang.Object")) {
                return;
            }
        }
        if syntax::has_modifier(node, "static") {
            return;
        }
        let Some(body) = ctx.parent(node) else {
            return;
        };
        let has_canonical = ctx.members(body).any(|m| {
            kind(m) == "method_declaration" && syntax::name_of(m, ctx.source()) == canonical
        });
        if has_canonical {
            return;
        }
        ctx.emit(
            name_node,
            format!("Rename to `{canonical}` (and add @Override): `{name}` does not override `Object.{canonical}`"),
        );
    }
}

/// `protected void finalize(X x) throws Throwable { …; super.finalize(); }`: written as a
/// finalizer, but with parameters, so the garbage collector never calls it. A `finalize(..)`
/// that is just a domain verb (no `throws Throwable`, no `super.finalize()`) is left alone.
fn check_finalizer_with_parameters<'t>(node: Node<'t>, name: Node<'t>, ctx: &Ctx<'t>) {
    if syntax::parameters(node).next().is_none() || syntax::has_modifier(node, "static") {
        return;
    }
    if node.field(Field::Type).map(|t| kind(t)) != Some("void_type") {
        return;
    }
    let throws_throwable = named_children(node)
        .find(|c| kind(*c) == "throws")
        .is_some_and(|t| {
            named_children(t).any(|e| matches!(ctx.text(e), "Throwable" | "java.lang.Throwable"))
        });
    let calls_super = node
        .field(Field::Body)
        .is_some_and(|b| ctx.text(b).contains("super.finalize()"));
    if !throws_throwable && !calls_super {
        return;
    }
    ctx.emit(
        name,
        "Remove the parameters (or rename the method): the garbage collector only calls `finalize()` without parameters",
    );
}
