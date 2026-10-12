//! `java/static-init-uses-subclass` — a class whose static initialization creates or calls
//! into one of its own subclasses.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["concurrency", "initialization"],
    remediation_minutes: 20,
    description: r#"Detects a class whose static initialization — a static field initializer or
a `static { }` block — creates an instance of one of its subclasses (`new Sub()`) or calls a
static method of it, when the subclass is declared in the same file and can be used from
elsewhere (it is not `private`).

**Why it matters.** Initializing a class first initializes its superclass. If one thread
starts with the superclass and another with the subclass, each holds the initialization of
one class while waiting for the other: the JVM deadlocks, typically once in a while at
start-up under load. Even on a single thread, starting with the subclass runs the
superclass initializer while the subclass is half initialized, so the new instance sees
default values in the subclass's static fields.

**How to fix.** Move the instance out of the superclass: a holder class
(`class Defaults { static final Base INSTANCE = new Impl(); }`), a static factory method
creating it lazily, or make the subclass `private` so that only the superclass can trigger
its initialization.

```java
// Noncompliant
public abstract class Codec {
    public static final Codec DEFAULT = new Utf8Codec();
}
class Utf8Codec extends Codec { }

// Compliant
public abstract class Codec {
    public static Codec defaultCodec() { return Holder.DEFAULT; }
    private static final class Holder { static final Codec DEFAULT = new Utf8Codec(); }
}
```

Not reported: anonymous subclasses and `private` nested subclasses (only the superclass can
reach them), enum constants with bodies, interfaces, and subclasses declared in other files
(unknown to the file-local analysis)."#,
    ..RuleMeta::new(
        "java/static-init-uses-subclass",
        "java",
        "Static initialization uses a subclass",
    )
};

#[derive(Default)]
pub struct StaticInitUsesSubclass<'t> {
    /// Named classes of the file: declaration and simple name.
    classes: Vec<(Node<'t>, &'t str)>,
    /// `new T(..)` / `T.m(..)` in static initialization: (class being initialized, the use,
    /// the named type).
    uses: Vec<(Node<'t>, Node<'t>, Node<'t>)>,
    gate: util::Gate,
}

impl<'t> JavaRule<'t> for StaticInitUsesSubclass<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "class_declaration",
            "field_declaration",
            "static_initializer",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !self.gate.open(ctx, "extends") {
            return;
        }
        match kind(node) {
            "class_declaration" => {
                self.classes
                    .push((node, syntax::name_of(node, ctx.source())));
                return;
            }
            "field_declaration" if !syntax::has_modifier(node, "static") => return,
            _ => {}
        }
        // The member must belong to a named class.
        let Some(body) = ctx.parent(node) else {
            return;
        };
        if kind(body) != "class_body" {
            return;
        }
        let Some(owner) = ctx.parent(body).filter(|d| kind(*d) == "class_declaration") else {
            return;
        };
        let roots: Vec<Node<'t>> = if kind(node) == "static_initializer" {
            vec![node]
        } else {
            syntax::declarators(node)
                .filter_map(|d| d.field(Field::Value))
                .collect()
        };
        for root in roots {
            let mut scan = |n: Node<'t>| match kind(n) {
                "object_creation_expression" => {
                    if syntax::child_of_kind(n, "class_body").is_none() {
                        if let Some(ty) = n.field(Field::Type) {
                            self.uses.push((owner, n, ty));
                        }
                    }
                }
                "method_invocation" => {
                    if let Some(obj) = n.field(Field::Object) {
                        if kind(obj) == "identifier"
                            && ctx.text(obj).starts_with(|c: char| c.is_ascii_uppercase())
                        {
                            self.uses.push((owner, n, obj));
                        }
                    }
                }
                _ => {}
            };
            // Deferred code (lambdas, anonymous class bodies) does not run during initialization.
            let enter = |n: Node<'t>| !matches!(kind(n), "lambda_expression" | "class_body");
            if !enter(root) {
                continue;
            }
            if matches!(
                kind(root),
                "object_creation_expression" | "method_invocation"
            ) {
                scan(root);
            }
            util::visit(root, enter, &mut scan);
        }
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        if self.uses.is_empty() {
            return;
        }
        for &(owner, at, ty) in &self.uses {
            let name = util::last_segment(ctx.text(type_base(ty)));
            let Some(&(sub, _)) = self.classes.iter().find(|(_, n)| *n == name) else {
                continue;
            };
            if sub.id() == owner.id()
                || syntax::has_modifier(sub, "private")
                || !extends_in_file(ctx, &self.classes, sub, owner)
            {
                continue;
            }
            // The name must really denote that class here (not a same-named import).
            let (Some(sub_fqn), Some(owner_name)) = (ctx.class_fqn(sub), owner.field(Field::Name))
            else {
                continue;
            };
            if ctx.resolve_type_name(name, at).as_deref() != Some(sub_fqn.as_str()) {
                continue;
            }
            let what = if kind(at) == "object_creation_expression" {
                "creates an instance of"
            } else {
                "calls into"
            };
            ctx.emit(
                at,
                format!(
                    "Move this out of the static initialization of `{}`: it {what} its subclass `{name}`, which can deadlock class initialization",
                    ctx.text(owner_name)
                ),
            );
        }
    }
}

/// The type name without type arguments.
fn type_base(ty: Node<'_>) -> Node<'_> {
    if kind(ty) == "generic_type" {
        syntax::first_named_child(ty).unwrap_or(ty)
    } else {
        ty
    }
}

/// Whether `sub` extends `sup`, directly or through classes of this file.
fn extends_in_file<'t>(
    ctx: &Ctx<'t>,
    classes: &[(Node<'t>, &'t str)],
    sub: Node<'t>,
    sup: Node<'t>,
) -> bool {
    let sup_name = syntax::name_of(sup, ctx.source());
    let mut cur = sub;
    for _ in 0..8 {
        let Some(sc) = cur.field(Field::Superclass) else {
            return false;
        };
        let Some(ty) = syntax::first_named_child(sc) else {
            return false;
        };
        let name = util::last_segment(ctx.text(type_base(ty)));
        if name == sup_name {
            return ctx
                .resolve_type(ty)
                .and_then(|t| t.fqn().map(str::to_string))
                .is_some_and(|f| ctx.class_fqn(sup).as_deref() == Some(f.as_str()));
        }
        let Some(&(next, _)) = classes.iter().find(|(_, n)| *n == name) else {
            return false;
        };
        cur = next;
    }
    false
}
