//! `java/compare-to-without-equals` — a `Comparable` class that defines `compareTo` but keeps
//! the identity `equals` of `Object`.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["equality", "collections"],
    remediation_minutes: 15,
    description: r#"Detects classes that implement `Comparable` with their own `compareTo` but do
not override `equals` (and `hashCode`), so they inherit the identity comparison of `Object`.

**Why it matters.** The JDK collections do not agree on how such objects are compared:
`TreeSet`, `TreeMap` and `Collections.binarySearch` use `compareTo`, while `HashSet`,
`HashMap`, `List.contains` and `List.remove` use `equals`. Two distinct objects that compare
as `0` are one element in a `TreeSet` and two in a `HashSet`; `list.remove(x)` does not
remove an element that sorts exactly like `x`. The `Comparable` contract strongly recommends
that `compareTo` be consistent with `equals` for this reason.

**How to fix.** Override `equals` (and `hashCode`) using the same fields that `compareTo`
compares, or document why the natural ordering is deliberately inconsistent with equality.

```java
// Noncompliant
class Version implements Comparable<Version> {
    int major, minor;
    public int compareTo(Version o) { … }
}

// Compliant
class Version implements Comparable<Version> {
    int major, minor;
    public int compareTo(Version o) { … }
    @Override public boolean equals(Object o) { … }
    @Override public int hashCode() { return Objects.hash(major, minor); }
}
```

Only classes that implement `Comparable` directly are checked, and only when every superclass
is known not to define `equals`: no `extends`, a class of this file (checked the same way),
or `Object`, `Number`, `Thread` or an exception class. Abstract classes, records, enums and
classes with Lombok annotations are skipped."#,
    ..RuleMeta::new(
        "java/compare-to-without-equals",
        "java",
        "compareTo() without equals()",
    )
};

pub struct CompareToWithoutEquals;

/// JDK superclasses that do not override `equals`.
const PLAIN_JDK_SUPERS: &[&str] = &[
    "java.lang.Object",
    "java.lang.Number",
    "java.lang.Thread",
    "java.lang.Throwable",
    "java.lang.Exception",
    "java.lang.RuntimeException",
    "java.lang.Error",
];

fn declares<'t>(ctx: &Ctx<'t>, class: Node<'t>, name: &str, params: usize) -> bool {
    syntax::type_body(class).is_some_and(|b| {
        ctx.members(b).any(|m| {
            kind(m) == "method_declaration"
                && syntax::name_of(m, ctx.source()) == name
                && syntax::parameters(m).count() == params
        })
    })
}

/// Whether `class` and all its superclasses provably inherit `Object.equals`.
fn inherits_identity_equals<'t>(ctx: &Ctx<'t>, class: Node<'t>, depth: u32) -> bool {
    if depth > 8 || declares(ctx, class, "equals", 1) || ctx.has_lombok(class) {
        return false;
    }
    let Some(sup) = class.field(Field::Superclass) else {
        return true;
    };
    let Some(ty) = syntax::first_named_child(sup) else {
        return false;
    };
    let base = if kind(ty) == "generic_type" {
        syntax::first_named_child(ty)
    } else {
        Some(ty)
    };
    let Some(base) = base else { return false };
    let Some(fqn) = ctx.resolve_type_name(ctx.text(base), base) else {
        return false;
    };
    if PLAIN_JDK_SUPERS.contains(&fqn.as_str()) {
        return true;
    }
    // A superclass declared in this file.
    let Some(t) = ctx.resolve_type(ty) else {
        return false;
    };
    let Some(parent) = local_class(ctx, class, t.fqn().unwrap_or("")) else {
        return false;
    };
    kind(parent) == "class_declaration" && inherits_identity_equals(ctx, parent, depth + 1)
}

/// The class declaration of this file with the given fully-qualified name, searched among the
/// top-level declarations and their member types (bounded: members only, not method bodies).
fn local_class<'t>(ctx: &Ctx<'t>, from: Node<'t>, fqn: &str) -> Option<Node<'t>> {
    if fqn.is_empty() {
        return None;
    }
    let mut stack: Vec<Node<'t>> = named_children(ctx.root())
        .filter(|n| syntax::is_type_declaration(*n))
        .collect();
    let mut steps = 0;
    while let Some(d) = stack.pop() {
        steps += 1;
        if steps > 512 {
            return None;
        }
        if d.id() != from.id() && ctx.class_fqn(d).as_deref() == Some(fqn) {
            return Some(d);
        }
        if let Some(b) = syntax::type_body(d) {
            stack.extend(ctx.members(b).filter(|m| syntax::is_type_declaration(*m)));
        }
    }
    None
}

impl<'t> JavaRule<'t> for CompareToWithoutEquals {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["class_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(interfaces) = node.field(Field::Interfaces) else {
            return;
        };
        if !ctx.text(interfaces).contains("Comparable") {
            return;
        }
        if syntax::has_modifier(node, "abstract") {
            return;
        }
        // `implements Comparable<…>` resolving to the JDK interface.
        let implements = named_children(interfaces)
            .flat_map(named_children)
            .any(|t| {
                let base = if kind(t) == "generic_type" {
                    syntax::first_named_child(t)
                } else {
                    Some(t)
                };
                base.is_some_and(|b| {
                    ctx.resolve_type_name(ctx.text(b), b).as_deref() == Some("java.lang.Comparable")
                })
            });
        if !implements {
            return;
        }
        let Some(compare) = syntax::type_body(node).and_then(|b| {
            ctx.members(b).find(|m| {
                kind(*m) == "method_declaration"
                    && syntax::name_of(*m, ctx.source()) == "compareTo"
                    && syntax::parameters(*m).count() == 1
                    && m.field(Field::Body).is_some()
            })
        }) else {
            return;
        };
        if !inherits_identity_equals(ctx, node, 0) {
            return;
        }
        let (Some(name), Some(cmp)) = (node.field(Field::Name), compare.field(Field::Name)) else {
            return;
        };
        ctx.emit(
            name,
            format!(
                "`{}` defines compareTo() but not equals(): sorted and hash-based collections will disagree on duplicates",
                ctx.text(name)
            ),
        )
        .secondary(cmp, "compareTo defined here");
    }
}
