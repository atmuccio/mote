//! `java/inner-class-could-be-static` — a non-static member class that never uses the
//! instance of its enclosing class.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["memory", "design"],
    remediation_minutes: 2,
    description: r#"Detects private or package-private member classes that are not `static` although
nothing in them uses the enclosing instance: no field or method of the enclosing class, no
`Outer.this`, no creation of other inner classes.

**Why it matters.** Every instance of an inner (non-static) class carries a hidden reference to
the object that created it. When the class does not need that object, the reference only
costs memory and keeps the enclosing object alive as long as any inner instance exists (a
classic leak with listeners, caches and tasks handed to other threads). It also means the
class can only be created from an instance of the enclosing class, and misleads readers into
looking for a dependency that is not there.

**How to fix.** Declare the class `static`.

```java
// Noncompliant
class Cache {
    private class Entry {          // holds a hidden Cache reference it never uses
        String key;
        long expires;
    }
}

// Compliant
class Cache {
    private static class Entry {
        String key;
        long expires;
    }
}
```

The check is conservative: any name in the class that matches an instance member of an
enclosing class counts as a use, as does any reference to the enclosing class's type
parameters. Classes are skipped when an enclosing class extends another class or implements
interfaces (inherited members cannot be seen), when they are public or protected, annotated
`@Nested` (JUnit requires those to be inner), or nested in an interface, a local or an
anonymous class. Test code is not checked (a hidden reference in a test helper costs
nothing), nor are files that use Lombok, whose generated accessors cannot be seen."#,
    ..RuleMeta::new(
        "java/inner-class-could-be-static",
        "java",
        "Inner class could be static",
    )
};

pub struct InnerClassCouldBeStatic;

/// Names of the instance members and non-static member classes of `class`, and whether
/// its members can all be seen (no supertypes).
fn instance_members<'t>(ctx: &Ctx<'t>, class: Node<'t>, out: &mut Vec<&'t str>) -> bool {
    if class.field(Field::Superclass).is_some() || class.field(Field::Interfaces).is_some() {
        return false;
    }
    let src = ctx.source();
    if kind(class) == "record_declaration" {
        for p in syntax::parameters(class) {
            out.push(syntax::name_of(p, src));
        }
    }
    let Some(body) = syntax::type_body(class) else {
        return false;
    };
    for m in ctx.members(body) {
        match kind(m) {
            "field_declaration" if !syntax::has_modifier(m, "static") => {
                out.extend(syntax::declarators(m).map(|d| syntax::name_of(d, src)));
            }
            "method_declaration" if !syntax::has_modifier(m, "static") => {
                out.push(syntax::name_of(m, src));
            }
            "class_declaration" if !syntax::has_modifier(m, "static") => {
                out.push(syntax::name_of(m, src));
            }
            _ => {}
        }
    }
    true
}

/// The type declaration `member` is a direct member of (through `class_body`, or the
/// `enum_body_declarations` of an `enum_body`); `None` for local, anonymous and top-level
/// classes.
fn enclosing_type<'t>(ctx: &Ctx<'t>, member: Node<'t>) -> Option<Node<'t>> {
    let body = ctx.parent(member)?;
    match kind(body) {
        "class_body" => {
            let outer = ctx.parent(body)?;
            syntax::is_type_declaration(outer).then_some(outer)
        }
        "enum_body_declarations" => {
            let enum_body = ctx.parent(body)?;
            let outer = ctx.parent(enum_body)?;
            (kind(outer) == "enum_declaration").then_some(outer)
        }
        _ => None,
    }
}

/// Type parameter names of `class`.
fn type_params<'t>(ctx: &Ctx<'t>, class: Node<'t>, out: &mut Vec<&'t str>) {
    if let Some(tp) = class.field(Field::TypeParameters) {
        for p in named_children(tp) {
            if let Some(n) = named_children(p)
                .find(|c| kind(*c) == "identifier" || kind(*c) == "type_identifier")
            {
                out.push(ctx.text(n));
            }
        }
    }
}

impl<'t> JavaRule<'t> for InnerClassCouldBeStatic {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["class_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if ctx.is_test()
            || syntax::has_modifier(node, "static")
            || syntax::has_modifier(node, "public")
            || syntax::has_modifier(node, "protected")
            || ctx.imports().iter().any(|i| i.path.starts_with("lombok"))
        {
            return;
        }
        let src = ctx.source();
        if syntax::annotations(node).any(|a| syntax::annotation_name(a, src) == "Nested") {
            return;
        }
        // The chain of enclosing classes, all named member or top-level classes/enums/records.
        let mut members: Vec<&'t str> = Vec::new();
        let mut tparams: Vec<&'t str> = Vec::new();
        let mut cur = node;
        let mut levels = 0;
        loop {
            // Deeply nested classes: give up rather than scan every level for each class.
            levels += 1;
            if levels > 8 {
                return;
            }
            let Some(outer) = enclosing_type(ctx, cur) else {
                return;
            };
            if !matches!(
                kind(outer),
                "class_declaration" | "enum_declaration" | "record_declaration"
            ) {
                return;
            }
            if !instance_members(ctx, outer, &mut members) {
                return;
            }
            type_params(ctx, outer, &mut tparams);
            match ctx.parent(outer) {
                Some(p) if kind(p) == "program" => break,
                _ => {
                    // A static member class (or an enum or record, implicitly static) cuts the
                    // chain of enclosing instances.
                    if syntax::has_modifier(outer, "static") || kind(outer) != "class_declaration" {
                        break;
                    }
                    cur = outer;
                }
            }
        }
        let Some(class_body) = node.field(Field::Body) else {
            return;
        };
        // The class's own superclass may be an inner class of the enclosing one.
        if let Some(sup) = node.field(Field::Superclass) {
            let t = ctx.text(sup);
            if members.iter().any(|m| t.contains(m)) || tparams.iter().any(|p| t.contains(p)) {
                return;
            }
        }
        if let Some(i) = node.field(Field::Interfaces) {
            if tparams.iter().any(|p| ctx.text(i).contains(p)) {
                return;
            }
        }
        let own = syntax::name_of(node, src);
        if uses_enclosing(ctx, class_body, own, &members, &tparams) {
            return;
        }
        let Some(name) = node.field(Field::Name) else {
            return;
        };
        ctx.emit(
            name,
            format!("Make `{own}` static: it never uses the enclosing instance, yet each `{own}` keeps a hidden reference to it"),
        );
    }
}

/// Whether anything in `body` may use the enclosing instance: an unqualified name of one of its
/// instance members, `Outer.this` / `Outer.super`, a `new` of one of its inner classes, or one
/// of its type parameters. Iterative, bounded; parents are never looked up.
fn uses_enclosing<'t>(
    ctx: &Ctx<'t>,
    body: Node<'t>,
    own: &str,
    members: &[&'t str],
    tparams: &[&'t str],
) -> bool {
    let member = |n: Node<'t>| {
        let t = ctx.text(n);
        t != own && members.contains(&t)
    };
    let mut stack: Vec<Node<'t>> = named_children(body).collect();
    let mut budget = 200_000u32;
    while let Some(n) = stack.pop() {
        if budget == 0 {
            return true;
        }
        budget -= 1;
        match kind(n) {
            "ERROR" => return true,
            "identifier" => {
                if member(n) {
                    return true;
                }
            }
            "type_identifier" => {
                if tparams.contains(&ctx.text(n)) {
                    return true;
                }
            }
            "field_access" => {
                // `Outer.this.x`, `Outer.super.m()`; `obj.name` does not use the outer instance.
                if n.field(Field::Field)
                    .is_some_and(|f| matches!(kind(f), "this" | "super"))
                {
                    return true;
                }
                stack.extend(n.field(Field::Object));
            }
            "method_invocation" => {
                let object = n.field(Field::Object);
                if object.is_none() && n.field(Field::Name).is_some_and(member) {
                    return true;
                }
                stack.extend(object);
                stack.extend(n.field(Field::TypeArguments));
                stack.extend(n.field(Field::Arguments));
            }
            "method_reference" => {
                // Only the receiver (`this::m` refers to the inner class itself).
                stack.extend(syntax::first_named_child(n));
            }
            "object_creation_expression" => {
                if let Some(t) = n.field(Field::Type) {
                    let base = if kind(t) == "generic_type" {
                        syntax::first_named_child(t)
                    } else {
                        Some(t)
                    };
                    if base.is_some_and(member) {
                        return true;
                    }
                    stack.push(t);
                }
                stack.extend(n.field(Field::Arguments));
                stack.extend(syntax::type_body(n));
            }
            _ => stack.extend(named_children(n)),
        }
    }
    false
}
