//! `java/serialization-constructor-missing` — classes that serialization cannot re-create:
//! an `Externalizable` without a public no-argument constructor, or a `Serializable` class whose
//! non-serializable superclass (in this file) has no no-argument constructor.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["serialization"],
    remediation_minutes: 5,
    description: r#"Detects classes that Java serialization can write but not read back:

- a class implementing `Externalizable` without a `public` constructor taking no arguments
  (also when the only constructor is the implicit one of a non-public class);
- a class implementing `Serializable` whose superclass, declared in the same file, is not
  serializable and has no non-private constructor taking no arguments.

**Why it matters.** Deserialization re-creates the object before restoring its state. For an
`Externalizable` it calls the public no-argument constructor, then `readExternal`. For a
`Serializable` class it runs the no-argument constructor of the first non-serializable
superclass. When that constructor is missing or inaccessible, writing works, and every read
fails with `InvalidClassException: no valid constructor`: the problem shows only when the
stored data is needed.

**How to fix.** Add the constructor (`public Foo() {}` for an `Externalizable`, an accessible
no-argument constructor in the non-serializable superclass), or make the superclass
serializable.

```java
// Noncompliant
class Session implements Externalizable {
    Session(String id) { … }                 // no public no-arg constructor
    …
}

// Compliant
public class Session implements Externalizable {
    public Session() { }
    Session(String id) { … }
    …
}
```

Abstract classes are not checked, and the superclass case is reported only when the superclass
is declared in the same file without a superclass or interfaces of its own (it provably isn't
serializable)."#,
    ..RuleMeta::new(
        "java/serialization-constructor-missing",
        "java",
        "Serializable class that cannot be deserialized",
    )
};

pub struct SerializationConstructorMissing;

/// The `type_list` types of a class's `implements` clause.
fn interfaces<'t>(class: Node<'t>) -> impl Iterator<Item = Node<'t>> {
    class
        .field(Field::Interfaces)
        .and_then(syntax::first_named_child)
        .into_iter()
        .flat_map(named_children)
}

fn implements<'t>(class: Node<'t>, fqn: &str, ctx: &Ctx<'t>) -> bool {
    let simple = fqn.rsplit('.').next().unwrap_or(fqn);
    interfaces(class).any(|t| {
        let base = if kind(t) == "generic_type" {
            syntax::first_named_child(t).unwrap_or(t)
        } else {
            t
        };
        let text = ctx.text(base);
        (text == simple || text == fqn) && ctx.resolve_type_name(text, base).as_deref() == Some(fqn)
    })
}

/// (constructors declared, whether one takes no parameters and satisfies `ok`)
fn no_arg_constructor<'t>(class: Node<'t>, ok: impl Fn(Node<'t>) -> bool) -> (usize, bool) {
    let Some(body) = syntax::type_body(class) else {
        return (0, false);
    };
    let mut count = 0;
    let mut found = false;
    for m in syntax::members(body) {
        if kind(m) != "constructor_declaration" {
            continue;
        }
        count += 1;
        if syntax::parameters(m).next().is_none() && ok(m) {
            found = true;
        }
    }
    (count, found)
}

impl<'t> JavaRule<'t> for SerializationConstructorMissing {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["class_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if node.field(Field::Interfaces).is_none() || syntax::has_modifier(node, "abstract") {
            return;
        }
        let Some(name) = node.field(Field::Name) else {
            return;
        };
        let text = ctx.text(node.field(Field::Interfaces).unwrap_or(node));
        if text.contains("Externalizable") && implements(node, "java.io.Externalizable", ctx) {
            let (count, public_no_arg) =
                no_arg_constructor(node, |c| syntax::has_modifier(c, "public"));
            let broken = if count == 0 {
                // The implicit constructor has the class's access.
                !implicitly_public(node, ctx)
            } else {
                !public_no_arg
            };
            if broken {
                ctx.emit(
                    name,
                    format!("Give `{}` a public no-argument constructor; deserializing an Externalizable calls it and fails without it", ctx.text(name)),
                );
            }
            return;
        }
        if !text.contains("Serializable") || !implements(node, "java.io.Serializable", ctx) {
            return;
        }
        let Some(parent) = same_file_superclass(node, ctx) else {
            return;
        };
        // The parent must provably not be serializable: no supertypes of its own.
        if parent.field(Field::Superclass).is_some() || parent.field(Field::Interfaces).is_some() {
            return;
        }
        let (count, accessible) =
            no_arg_constructor(parent, |c| !syntax::has_modifier(c, "private"));
        if count == 0 || accessible {
            return;
        }
        let parent_name = syntax::name_of(parent, ctx.source());
        ctx.emit(
            name,
            format!("Add a no-argument constructor to `{parent_name}` (or make it Serializable); deserializing `{}` needs one and fails without it", ctx.text(name)),
        );
    }
}

/// Whether a class's implicit default constructor is `public`: the class is `public`, or a
/// member of an interface (implicitly public).
fn implicitly_public<'t>(class: Node<'t>, ctx: &Ctx<'t>) -> bool {
    if syntax::has_modifier(class, "public") {
        return true;
    }
    ctx.parent(class)
        .is_some_and(|b| kind(b) == "interface_body")
}

/// The class declared in this file that `class` extends, when its name is unique here.
fn same_file_superclass<'t>(class: Node<'t>, ctx: &Ctx<'t>) -> Option<Node<'t>> {
    let sup = class.field(Field::Superclass)?;
    let ty = syntax::first_named_child(sup)?;
    let base = if kind(ty) == "generic_type" {
        syntax::first_named_child(ty)?
    } else {
        ty
    };
    let t = ctx.resolve_type(base)?;
    let fqn = t.fqn()?.to_string();
    // Find the declaration among the file's type declarations (bounded walk of type bodies).
    let root = ctx.root();
    let mut found = None;
    let mut stack = vec![root];
    let mut budget = 4096u32;
    while let Some(n) = stack.pop() {
        for c in named_children(n) {
            budget = budget.saturating_sub(1);
            if budget == 0 {
                return None;
            }
            if kind(c) == "class_declaration" {
                if ctx.class_fqn(c).as_deref() == Some(fqn.as_str()) {
                    if found.is_some() {
                        return None;
                    }
                    found = Some(c);
                }
                if let Some(b) = syntax::type_body(c) {
                    stack.push(b);
                }
            } else if syntax::is_type_declaration(c) {
                if let Some(b) = syntax::type_body(c) {
                    stack.push(b);
                }
            }
        }
    }
    found
}
