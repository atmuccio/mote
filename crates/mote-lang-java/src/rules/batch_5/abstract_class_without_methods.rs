//! `java/abstract-class-without-methods` — an abstract class with no methods at all.

use super::util::{self, Mods, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["design"],
    remediation_minutes: 5,
    params: &[ParamSpec {
        name: "interface_candidates",
        default: ParamDefault::Bool(true),
        description: "Also report abstract classes that only declare public abstract methods (no \
                      state, constructors, concrete or non-public methods): an interface says \
                      the same and leaves implementors free to extend another class.",
    }],
    description: r#"Detects `abstract` classes that declare no methods and no constructors, and
(with `interface_candidates`, on by default) abstract classes that are an interface in
disguise: no fields, constructors, initializers or concrete methods, only `public abstract`
methods.

**Why it matters.** `abstract` promises behaviour that subclasses complete. A class without
any method has nothing to complete: the keyword is only used to prevent instantiation, which
misleads readers looking for the abstract part.

**How to fix.** If the class just holds data, make it a concrete class (with a private or
protected constructor if instances should not be created directly). If it only groups
constants or nested types, make it a `final` class with a private constructor.

```java
// Noncompliant
public abstract class Defaults {
    public static final int TIMEOUT = 30;
}

// Compliant
public final class Defaults {
    public static final int TIMEOUT = 30;

    private Defaults() { }
}

// Noncompliant: nothing but a contract
public abstract class Shape {
    public abstract double area();
}

// Compliant
public interface Shape {
    double area();
}
```

A `private` constructor does not count as a method: an abstract class whose only constructor is
private can neither be instantiated nor extended outside its own body.

Not reported: classes that extend a class or implement an interface (they may leave
inherited abstract methods to subclasses), generic classes (the "type token" idiom relies on
anonymous subclasses), `sealed` classes, annotated classes, and classes with a private
constructor that are extended inside their own body (`extends Shape` or `new Shape() { … }`).
An abstract class with a `protected` or package-private abstract method, a field, a
constructor, or a concrete method is not an interface candidate: an interface could not keep
it."#,
    ..RuleMeta::new(
        "java/abstract-class-without-methods",
        "java",
        "Abstract class without methods",
    )
};

pub struct AbstractClassWithoutMethods;

/// An abstract class that only declares `public abstract` methods (at least one): no fields,
/// constructors, initializers, concrete or non-public methods, and no annotations on its
/// methods beyond `@Override`-like markers (frameworks may read those).
fn interface_candidate<'t>(ctx: &Ctx<'t>, class: Node<'t>) -> bool {
    let mut methods = 0;
    for m in util::class_members(ctx, class) {
        match kind(m) {
            "method_declaration" => {
                let mods = Mods::of(m);
                if !mods.has("public")
                    || !mods.has("abstract")
                    || m.field(Field::Body).is_some()
                    || syntax::annotations(m).any(|a| {
                        !matches!(
                            syntax::annotation_name(a, ctx.source()),
                            "Override" | "Deprecated" | "Nullable" | "NonNull" | "Nonnull"
                        )
                    })
                {
                    return false;
                }
                methods += 1;
            }
            // Nested types are allowed in interfaces too.
            "class_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "record_declaration"
            | "annotation_type_declaration" => {}
            _ => return false,
        }
    }
    methods > 0
}

impl<'t> JavaRule<'t> for AbstractClassWithoutMethods {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["class_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let mods = Mods::of(node);
        if !mods.has("abstract") || mods.has("sealed") || mods.annotated() {
            return;
        }
        // Everything but the type parameters: an interface can be generic, but not extend a
        // class.
        if ctx.param_bool("interface_candidates")
            && util::fld(node, F.superclass).is_none()
            && !named_children(node).any(|c| kind(c) == "permits")
            // Local interfaces need Java 16.
            && ctx.parent(node).is_none_or(|p| kind(p) != "block")
            && interface_candidate(ctx, node)
        {
            if let Some(name) = util::fld(node, F.name) {
                ctx.emit(
                    name,
                    format!(
                        "Class `{}` only declares public abstract methods; make it an interface",
                        ctx.text(name)
                    ),
                );
            }
            return;
        }
        if util::fld(node, F.superclass).is_some()
            || named_children(node)
                .any(|c| matches!(kind(c), "super_interfaces" | "type_parameters" | "permits"))
        {
            return;
        }
        let mut private_constructor = false;
        for m in util::class_members(ctx, node) {
            match kind(m) {
                "method_declaration" => return,
                "constructor_declaration" => {
                    if !Mods::of(m).has("private") {
                        return;
                    }
                    private_constructor = true;
                }
                _ => {}
            }
        }
        let Some(name) = util::fld(node, F.name) else {
            return;
        };
        // With a private constructor, only the class's own nested classes can extend it.
        if private_constructor {
            let text = util::fld(node, F.body).map_or("", |b| ctx.text(b));
            let class = ctx.text(name);
            let extended = util::word_occurrences(text, class, 0, text.len()).any(|at| {
                let before = text[..at].trim_end();
                before.ends_with("extends") || before.ends_with("new")
            });
            if extended {
                return;
            }
        }
        ctx.emit(
            name,
            format!(
                "Class `{}` is abstract but declares no methods; make it concrete or final",
                ctx.text(name)
            ),
        );
    }
}
