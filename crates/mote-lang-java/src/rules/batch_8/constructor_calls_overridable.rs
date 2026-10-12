//! `java/constructor-calls-overridable` — a constructor calls a method of its class that a
//! subclass can override.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["inheritance", "initialization"],
    remediation_minutes: 10,
    description: r#"Detects constructors of non-final classes that call a method of the same class
which a subclass can override (not `private`, `static` or `final`).

**Why it matters.** Java runs the superclass constructor before the subclass's field
initializers and constructor body. If the superclass constructor calls an overridable
method and a subclass overrides it, the override runs on a half-built object: its fields
still hold `null`, `0` or `false`, so it can throw a `NullPointerException` or quietly
compute with defaults, and anything it stores may be overwritten by the subclass's own
initializers right afterwards.

**How to fix.** Make the called method `private`, `static` or `final`, make the class
`final`, or move the call out of the constructor (a factory method that constructs and then
initializes).

```java
// Noncompliant
class Widget {
    Widget() {
        layout();              // Button.layout() runs before Button's fields are set
    }
    protected void layout() { … }
}

// Compliant
class Widget {
    Widget() {
        layoutBase();
    }
    private void layoutBase() { … }
    protected void layout() { layoutBase(); }
}
```

Off by default: whether it matters depends on whether subclasses exist, which a single file
cannot tell. Only calls made by the constructor's own statements (a call statement, the right side
of an assignment, a local's initializer; not calls in nested blocks or lambdas), without a
receiver or on `this`, to methods declared in the same
class are considered; constructors (or classes) marked `@SuppressWarnings("this-escape")`,
the name javac's lint uses for this warning, are skipped, as are private nested classes that
nothing in the file extends, classes whose
constructors are all private (no subclass outside the
class is possible), enums and records."#,
    ..RuleMeta::new(
        "java/constructor-calls-overridable",
        "java",
        "Constructor calls an overridable method",
    )
};

/// A method of the class: name, arity, and whether a subclass can override it.
type MethodInfo<'t> = (&'t str, usize, bool);

/// Per-class facts, computed once for all constructors of a class.
#[derive(Default)]
pub struct ConstructorCallsOverridable<'t> {
    /// Class id, whether it can have subclasses, and its methods.
    cache: Option<(usize, bool, Vec<MethodInfo<'t>>)>,
}

fn overridable(m: Node<'_>) -> bool {
    !syntax::has_modifier(m, "private")
        && !syntax::has_modifier(m, "static")
        && !syntax::has_modifier(m, "final")
}

/// Whether a subclass of `class` can exist, and its methods.
fn class_facts<'t>(ctx: &Ctx<'t>, class: Node<'t>) -> (bool, Vec<MethodInfo<'t>>) {
    let src = ctx.source();
    let Some(class_body) = syntax::type_body(class) else {
        return (false, Vec::new());
    };
    if kind(class) != "class_declaration" || syntax::has_modifier(class, "final") {
        return (false, Vec::new());
    }
    // A private nested class can only be extended in this file.
    if syntax::has_modifier(class, "private")
        && !src.contains(&format!("extends {}", syntax::name_of(class, src)))
    {
        return (false, Vec::new());
    }
    let mut ctors = 0;
    let mut private_ctors = 0;
    let mut methods = Vec::new();
    for m in ctx.members(class_body) {
        match kind(m) {
            "constructor_declaration" => {
                ctors += 1;
                if syntax::has_modifier(m, "private") {
                    private_ctors += 1;
                }
            }
            "method_declaration" => methods.push((
                syntax::name_of(m, src),
                syntax::parameters(m).count(),
                overridable(m),
            )),
            _ => {}
        }
    }
    (ctors > private_ctors, methods)
}

impl<'t> JavaRule<'t> for ConstructorCallsOverridable<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["constructor_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        if !ctx.text(body).contains('(') || util::this_escape_suppressed(ctx, node) {
            return;
        }
        let Some(class) = ctx.enclosing_class(node) else {
            return;
        };
        if self.cache.as_ref().is_none_or(|c| c.0 != class.id()) {
            let (open, methods) = class_facts(ctx, class);
            self.cache = Some((class.id(), open, methods));
        }
        let Some((_, true, methods)) = self.cache.as_ref() else {
            return;
        };
        if methods.iter().all(|m| !m.2) {
            return;
        }
        let mut reported: Vec<&str> = Vec::new();
        // The calls the constructor's own statements make (not those in nested blocks): an
        // expression statement, the right side of an assignment, a local's initializer.
        for s in named_children(body) {
            let exprs: Vec<Node<'t>> = match kind(s) {
                "expression_statement" => syntax::first_named_child(s)
                    .map(|e| {
                        if kind(e) == "assignment_expression" {
                            e.field(Field::Right).unwrap_or(e)
                        } else {
                            e
                        }
                    })
                    .into_iter()
                    .collect(),
                "local_variable_declaration" => syntax::declarators(s)
                    .filter_map(|d| d.field(Field::Value))
                    .collect(),
                _ => continue,
            };
            for n in exprs {
                let n = syntax::strip_parens(n);
                if kind(n) != "method_invocation" {
                    continue;
                }
                if n.field(Field::Object).is_some_and(|o| kind(o) != "this") {
                    continue;
                }
                let Some(name_node) = n.field(Field::Name) else {
                    continue;
                };
                let name = ctx.text(name_node);
                if reported.contains(&name) {
                    continue;
                }
                let arity = util::arg_count(n);
                let mut candidates = methods.iter().filter(|m| m.0 == name && m.1 == arity);
                let Some(first) = candidates.next() else {
                    continue;
                };
                if !first.2 || candidates.any(|m| !m.2) {
                    continue;
                }
                reported.push(name);
                ctx.emit(
                    name_node,
                    format!("The constructor calls `{name}()`, which a subclass can override and would run on a half-built object"),
                );
            }
        }
    }
}
