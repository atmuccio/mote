//! `java/redundant-inheritance-declaration` — supertypes and abstract methods that a type
//! declares although it already has them: `extends Object`, an interface its superclass or
//! another listed interface already brings, an abstract method repeating an interface method.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["redundant", "clarity"],
    remediation_minutes: 2,
    description: r#"Detects parts of a type declaration that restate what the type already has:

* `extends Object` (every class extends `Object`);
* an interface in the `implements` (or an interface's `extends`) list that the superclass or
  another listed interface already implements, according to the declarations of this file and
  the JDK: `class Failure extends RuntimeException implements Serializable`,
  `interface Store extends Collection<Item>, List<Item>`, `enum Mode implements Comparable<Mode>`
  (every enum is `Comparable` and `Serializable`);
* an abstract method of an abstract class or an interface that repeats, word for word and
  without documentation, an abstract method of an interface declared in this file that the type
  lists: same name, parameter types, return type and `throws` clause.

**Why it matters.** The repetition adds nothing the compiler does not already know, yet it
makes the declaration look as if it did something: readers check why `Serializable` is listed
again, or whether the repeated method changes the contract. When the superclass later stops
implementing the interface, the stale mention silently keeps it.

**How to fix.** Remove the redundant part.

```java
// Noncompliant
public class Failure extends RuntimeException implements Serializable { … }
public abstract class Job implements Task {
    public abstract void run();   // Task declares: void run();
}

// Compliant
public class Failure extends RuntimeException { … }
public abstract class Job implements Task {
}
```

Not reported: supertypes declared in other files or libraries the resolver does not know (their
hierarchy cannot be checked); an interface that a JDK type gained only in a later release when
the project's Java release is older or unknown (`ExecutorService` is `AutoCloseable` from Java
19: `class Pool extends ThreadPoolExecutor implements AutoCloseable` is needed before); a repeated method that adds Javadoc or a comment, an annotation
other than `@Override`, a narrower return type or a shorter `throws` clause (those change or
document the contract); methods of generic interfaces or generic methods; abstract classes with
a superclass and types that also list an interface of another file (redeclaring the method
abstract can deliberately override an inherited implementation or `default` method); and
methods that another listed interface also declares."#,
    ..RuleMeta::new(
        "java/redundant-inheritance-declaration",
        "java",
        "Redundant supertype or inherited method declaration",
    )
};

/// An interface of this file whose abstract methods may be repeated.
struct Iface<'t> {
    decl: Node<'t>,
    body: Node<'t>,
    /// Filled in `finish`, and only when some type may repeat its methods.
    fqn: String,
}

/// A type declaring abstract methods, with the interfaces it lists.
struct Declarer<'t> {
    body: Node<'t>,
    listed: Vec<Node<'t>>,
}

#[derive(Default)]
pub struct RedundantInheritanceDeclaration<'t> {
    ifaces: Vec<Iface<'t>>,
    declarers: Vec<Declarer<'t>>,
}

/// The type nodes of a `super_interfaces` / `extends_interfaces` node (through its `type_list`).
fn listed_types(list: Node<'_>) -> Vec<Node<'_>> {
    let mut out = Vec::new();
    for c in named_children(list) {
        if kind(c) == "type_list" {
            out.extend(named_children(c));
        } else {
            out.push(c);
        }
    }
    out
}

/// The interfaces listed by a type declaration.
fn interfaces_of(decl: Node<'_>) -> Vec<Node<'_>> {
    let list = match kind(decl) {
        "interface_declaration" => syntax::child_of_kind(decl, "extends_interfaces"),
        _ => decl.field(Field::Interfaces),
    };
    list.map(listed_types).unwrap_or_default()
}

/// Source text of a node with all whitespace removed (for comparing types).
fn compact(ctx: &Ctx<'_>, n: Option<Node<'_>>) -> String {
    n.map(|n| ctx.text(n).split_whitespace().collect())
        .unwrap_or_default()
}

/// The parameters of a method without their names, compacted: modifiers, annotations and
/// types (`@NotNull String`, `final int`, `String...`).
fn param_types(ctx: &Ctx<'_>, m: Node<'_>) -> Vec<String> {
    let src = ctx.source();
    syntax::parameters(m)
        .map(|p| {
            let name = if kind(p) == "spread_parameter" {
                syntax::child_of_kind(p, "variable_declarator")
            } else {
                p.field(Field::Name)
            };
            let end = name.map_or(p.end_byte(), |n| n.start_byte());
            src.get(p.start_byte()..end)
                .unwrap_or("")
                .split_whitespace()
                .collect()
        })
        .collect()
}

/// The annotations of a method other than `@Override`, as sorted compacted texts.
fn annotation_texts(ctx: &Ctx<'_>, m: Node<'_>) -> Vec<String> {
    let src = ctx.source();
    let mut v: Vec<String> = syntax::annotations(m)
        .filter(|a| syntax::annotation_name(*a, src) != "Override")
        .map(|a| ctx.text(a).split_whitespace().collect())
        .collect();
    v.sort_unstable();
    v
}

/// Whether a comment on its own line directly precedes `node`.
fn documented<'t>(ctx: &Ctx<'t>, prev: Option<Node<'t>>) -> bool {
    let Some(p) = prev else { return false };
    if !matches!(kind(p), "line_comment" | "block_comment") {
        return false;
    }
    let src = ctx.source();
    let start = p.start_byte();
    let line_start = src[..start].rfind('\n').map_or(0, |i| i + 1);
    src[line_start..start].trim().is_empty()
}

impl<'t> RedundantInheritanceDeclaration<'t> {
    /// `extends Object` and interfaces already implemented by another listed supertype.
    fn check_supertypes(ctx: &Ctx<'t>, decl: Node<'t>, listed: &[Node<'t>]) {
        let superclass = if kind(decl) == "class_declaration" {
            decl.field(Field::Superclass)
                .and_then(syntax::first_named_child)
        } else {
            None
        };
        if let Some(s) = superclass {
            let text = ctx.text(s);
            if (text == "Object" || text == "java.lang.Object")
                && ctx.resolve_type_name(text, decl).as_deref() == Some("java.lang.Object")
            {
                if let Some(ext) = decl.field(Field::Superclass) {
                    ctx.emit(
                        ext,
                        "Remove `extends Object`: every class already extends Object",
                    );
                }
            }
        }
        if listed.is_empty() {
            return;
        }
        // The implicit superclass of an enum brings Comparable and Serializable.
        let implicit_enum =
            (kind(decl) == "enum_declaration").then(|| JType::class("java.lang.Enum"));
        // A superclass the resolver does not know cannot prove anything.
        let super_t = superclass.and_then(|s| ctx.resolve_type(s));
        // One interface and no known supertype: nothing to compare.
        if listed.len() == 1 && super_t.is_none() && implicit_enum.is_none() {
            return;
        }
        let resolved: Vec<Option<JType>> = listed.iter().map(|t| ctx.resolve_type(*t)).collect();
        for (j, t) in listed.iter().enumerate() {
            let Some(fqn) = resolved[j].as_ref().and_then(|r| r.fqn()) else {
                continue;
            };
            let mut by: Option<String> = None;
            if let Some(st) = &super_t {
                if st.fqn() != Some(fqn) && ctx.is_subtype_of(st, fqn) {
                    by = superclass.map(|s| ctx.text(s).to_string());
                }
            }
            if by.is_none() {
                if let Some(e) = &implicit_enum {
                    if ctx.is_subtype_of(e, fqn) {
                        by = Some("every enum".to_string());
                    }
                }
            }
            if by.is_none() {
                for (k, other) in resolved.iter().enumerate() {
                    let Some(o) = other else { continue };
                    if k != j && o.fqn() != Some(fqn) && ctx.is_subtype_of(o, fqn) {
                        by = Some(ctx.text(listed[k]).to_string());
                        break;
                    }
                }
            }
            let Some(by) = by else { continue };
            if needs_newer_release(ctx, fqn, super_t.as_ref(), &resolved) {
                continue;
            }
            let name = ctx.text(*t);
            let msg = if by == "every enum" {
                format!("Remove `{name}` from this list: every enum already implements it")
            } else {
                format!("Remove `{name}` from this list: `{by}` already implements it")
            };
            ctx.emit(*t, msg);
        }
    }

    /// An abstract method of `decl`'s body that repeats a method of a listed interface.
    fn check_methods(&self, ctx: &Ctx<'t>, d: &Declarer<'t>) {
        let src = ctx.source();
        // Every listed interface must be one of this file: an interface of elsewhere could
        // bring a default method that the redeclaration deliberately makes abstract again.
        let mut ifaces: Vec<&Iface<'t>> = Vec::new();
        for t in &d.listed {
            let Some(fqn) = ctx
                .resolve_type(*t)
                .and_then(|r| r.fqn().map(str::to_string))
            else {
                return;
            };
            match self.ifaces.iter().find(|i| i.fqn == fqn) {
                Some(i) => ifaces.push(i),
                None => return,
            }
        }
        let mut prev: Option<Node<'t>> = None;
        for m in syntax::children(d.body).filter(|c| c.is_named()) {
            let before = prev;
            prev = Some(m);
            if kind(m) != "method_declaration" || m.field(Field::Body).is_some() {
                continue;
            }
            if m.field(Field::TypeParameters).is_some() || documented(ctx, before) {
                continue;
            }
            if syntax::annotations(m).any(|a| syntax::annotation_name(a, src) != "Override") {
                continue;
            }
            let Some(name) = m.field(Field::Name) else {
                continue;
            };
            let name_text = ctx.text(name);
            let params = param_types(ctx, m);
            let ret = compact(ctx, m.field(Field::Type));
            let throws = compact(ctx, syntax::child_of_kind(m, "throws"));
            // Another listed interface declaring the same name (a default method, an
            // overload) makes the redeclaration a deliberate choice.
            let declaring = ifaces
                .iter()
                .filter(|i| {
                    ctx.members(i.body).any(|im| {
                        kind(im) == "method_declaration"
                            && im
                                .field(Field::Name)
                                .is_some_and(|n| ctx.text(n) == name_text)
                    })
                })
                .count();
            if declaring != 1 {
                continue;
            }
            for i in &ifaces {
                let repeated = ctx.members(i.body).find(|im| {
                    kind(*im) == "method_declaration"
                        && im.field(Field::Body).is_none()
                        && im.field(Field::TypeParameters).is_none()
                        && !syntax::has_modifier(*im, "static")
                        && !syntax::has_modifier(*im, "private")
                        && im
                            .field(Field::Name)
                            .is_some_and(|n| ctx.text(n) == name_text)
                        && param_types(ctx, *im) == params
                        && compact(ctx, im.field(Field::Type)) == ret
                        && compact(ctx, syntax::child_of_kind(*im, "throws")) == throws
                        && annotation_texts(ctx, *im).is_empty()
                });
                if repeated.is_some() {
                    let simple = i.fqn.rsplit('.').next().unwrap_or(&i.fqn);
                    ctx.emit(
                        name,
                        format!(
                            "Remove this abstract `{name_text}(..)`: it repeats the method of the interface `{simple}` without changing it"
                        ),
                    );
                    break;
                }
            }
        }
    }
}

impl<'t> JavaRule<'t> for RedundantInheritanceDeclaration<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "class_declaration",
            "interface_declaration",
            "enum_declaration",
            "record_declaration",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let listed = interfaces_of(node);
        Self::check_supertypes(ctx, node, &listed);
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        let is_interface = kind(node) == "interface_declaration";
        if is_interface && node.field(Field::TypeParameters).is_none() {
            self.ifaces.push(Iface {
                decl: node,
                body,
                fqn: String::new(),
            });
        }
        // A superclass could implement the method: redeclaring it abstract then forces
        // subclasses to implement it again, which is not redundant.
        let abstract_class = kind(node) == "class_declaration"
            && syntax::has_modifier(node, "abstract")
            && node.field(Field::Superclass).is_none();
        if (is_interface || abstract_class) && !listed.is_empty() {
            self.declarers.push(Declarer { body, listed });
        }
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        if self.ifaces.is_empty() || self.declarers.is_empty() {
            return;
        }
        for i in &mut self.ifaces {
            i.fqn = ctx.class_fqn(i.decl).unwrap_or_default();
        }
        for d in &self.declarers {
            self.check_methods(ctx, d);
        }
    }
}

/// JDK supertype edges added after Java 8 that the resolver's JDK model includes regardless of
/// the release: (type, supertype it gained, release). On an older release, or an unknown one,
/// listing that supertype again is required, not redundant.
const LATE_JDK_SUPERTYPES: &[(&str, &str, u32)] = &[(
    "java.util.concurrent.ExecutorService",
    "java.lang.AutoCloseable",
    19,
)];

/// Whether the redundancy of listing `fqn` may rest on a JDK edge the configured Java release
/// does not have yet (`class Pool extends ThreadPoolExecutor implements AutoCloseable` is
/// required before Java 19).
fn needs_newer_release(
    ctx: &Ctx<'_>,
    fqn: &str,
    super_t: Option<&JType>,
    listed: &[Option<JType>],
) -> bool {
    LATE_JDK_SUPERTYPES.iter().any(|&(ty, sup, since)| {
        sup == fqn
            && ctx.java_release().is_none_or(|r| r < since)
            && super_t
                .into_iter()
                .chain(listed.iter().flatten())
                .any(|t| t.fqn() != Some(fqn) && ctx.is_subtype_of(t, ty))
    })
}
