//! `java/wildcard-return-type` — bounded wildcards in return types, and type parameters bounded
//! by a final class.

use super::common::{self, Walk};
use crate::prelude::*;
use crate::resolve;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Incubating,
    tags: &["generics", "api-design"],
    remediation_minutes: 10,
    description: r#"Detects two ways of using generic bounds that give callers nothing:

* a bounded wildcard in a method's return type (`List<? extends Shape> shapes()`,
  `Map<String, ? super Integer> counts()`) when every `return` of the method gives one
  concrete type, visible in the declaration of the returned variable or field, or in a `new`
  expression;
* a type parameter whose bound is a final class, an enum or a record (`<T extends String>`):
  no other type can satisfy it.

**Why it matters.** Wildcards make a parameter flexible for the caller; in a return type they
do the opposite. Every caller receives a type it cannot write into, cannot pass where the
plain type is expected, and must repeat the wildcard in its own declarations. The method
knows what it returns and can say so. A type parameter bounded by a final type can only ever
be that type: the generic declaration only adds noise and casts.

**How to fix.** Return the concrete type (`List<Shape>`, or an unmodifiable view if the
wildcard was meant to prevent writes); replace the type parameter by its bound.

```java
// Noncompliant
public List<? extends Shape> shapes() { … }
public <T extends String> T first(List<T> items) { … }

// Compliant
public List<Shape> shapes() { … }
public String first(List<String> items) { … }
```

Not reported: methods that return values of several parameterizations, values whose own
declared type has the wildcard, or values whose type is not written down (results of calls,
`var`, diamonds) — the wildcard may then be needed; unbounded wildcards (`List<?>`) and any
wildcard inside `Class<…>` or
`Constructor<…>` (`Class<? extends Annotation>` is the idiomatic way to return a class
object); `? extends Object`; abstract, `default` and other interface methods, whose wildcard
lets implementations override them with a narrower return type; methods with `@Override`, and
non-private instance methods of types that have a supertype, which may be implementing a
signature they do not choose; bounds with several types."#,
    ..RuleMeta::new(
        "java/wildcard-return-type",
        "java",
        "Bounded wildcard in a return type or final type as a bound",
    )
};

#[derive(Default)]
pub struct WildcardReturnType<'t> {
    /// Type parameters with a single bound, checked in `finish`.
    params: Vec<(Node<'t>, Node<'t>)>,
    /// Final classes, enums and records declared in this file.
    final_types: Vec<String>,
}

/// Whether a declaration lists any supertype (`extends`, `implements`).
fn has_supertype(decl: Node<'_>) -> bool {
    match kind(decl) {
        "class_declaration" => {
            decl.field(Field::Superclass).is_some() || decl.field(Field::Interfaces).is_some()
        }
        "interface_declaration" => syntax::child_of_kind(decl, "extends_interfaces").is_some(),
        "enum_declaration" | "record_declaration" => decl.field(Field::Interfaces).is_some(),
        // Anonymous classes always implement or extend something.
        _ => true,
    }
}

/// Whether a generic type is a reflective handle (`Class<…>`, `Constructor<…>`), whose
/// bounded wildcards are the idiomatic way to describe "some subclass of".
fn is_class_type(ctx: &Ctx<'_>, generic: Node<'_>) -> bool {
    syntax::first_named_child(generic)
        .map(|b| ctx.text(b))
        .is_some_and(|t| {
            matches!(
                t,
                "Class" | "java.lang.Class" | "Constructor" | "java.lang.reflect.Constructor"
            )
        })
}

/// The first bounded wildcard of a return type that is not inside `Class<…>`.
fn bounded_wildcard<'t>(ctx: &Ctx<'t>, ty: Node<'t>, depth: u32) -> Option<Node<'t>> {
    if depth > 16 {
        return None;
    }
    match kind(ty) {
        "wildcard" => {
            let bound = syntax::children(ty)
                .filter(|c| c.is_named() && kind(*c) != "super")
                .last()?;
            let t = ctx.text(bound);
            (t != "Object" && t != "java.lang.Object").then_some(ty)
        }
        "generic_type" if is_class_type(ctx, ty) => None,
        "generic_type" | "type_arguments" | "array_type" => {
            named_children(ty).find_map(|c| bounded_wildcard(ctx, c, depth + 1))
        }
        _ => None,
    }
}

impl<'t> WildcardReturnType<'t> {
    fn check_method(ctx: &Ctx<'t>, m: Node<'t>) {
        let Some(ret) = m.field(Field::Type) else {
            return;
        };
        if !ctx.text(ret).contains('?') {
            return;
        }
        let src = ctx.source();
        if syntax::has_annotation(m, src, "Override") {
            return;
        }
        let Some(wildcard) = bounded_wildcard(ctx, ret, 0) else {
            return;
        };
        let free = syntax::has_modifier(m, "private") || syntax::has_modifier(m, "static");
        if !free {
            // Abstract and interface methods: the wildcard lets implementations return a
            // narrower type (`List<Circle>` for `List<? extends Shape>`).
            if m.field(Field::Body).is_none() || syntax::has_modifier(m, "default") {
                return;
            }
            let Some(owner) = ctx.enclosing_class(m) else {
                return;
            };
            if kind(owner) == "interface_declaration" || has_supertype(owner) {
                return;
            }
        }
        // Only when the method visibly returns one concrete type: a wildcard can be needed to
        // return values of several parameterizations (`Meta<Method>` or `Meta<Constructor>`),
        // or a value whose own declaration has the wildcard.
        let Some(concrete) = concrete_return(ctx, m) else {
            return;
        };
        ctx.emit(
            wildcard,
            format!(
                "Return `{concrete}` instead of a type with `{}`: every `return` gives that type, and the wildcard is passed on to every caller",
                ctx.text(wildcard)
            ),
        );
    }
}

/// The declared type node of what a returned expression refers to, when it has one we can read:
/// a variable, field or parameter (`list`, `this.list`), or `new X<A>(..)` without a diamond.
fn declared_type_node<'t>(ctx: &Ctx<'t>, e: Node<'t>) -> Option<Node<'t>> {
    let e = syntax::strip_parens(e);
    match kind(e) {
        "identifier" => {
            let d = ctx.declaration_of(e)?;
            if d.is_var {
                return None;
            }
            d.type_node
        }
        "field_access" => {
            let obj = e.field(Field::Object)?;
            if kind(obj) != "this" {
                return None;
            }
            declared_type_node(ctx, e.field(Field::Field)?)
        }
        "object_creation_expression" => {
            if syntax::child_of_kind(e, "class_body").is_some() {
                return None;
            }
            let t = e.field(Field::Type)?;
            let args = syntax::child_of_kind(t, "type_arguments")?;
            // A diamond (`new ArrayList<>()`) takes its arguments from the context.
            (named_children(args).next().is_some()).then_some(t)
        }
        _ => None,
    }
}

/// The one concrete type every `return` of the method gives: each returned expression is a
/// variable, field or creation whose written type has no wildcard, and all agree.
fn concrete_return<'t>(ctx: &Ctx<'t>, m: Node<'t>) -> Option<String> {
    let body = m.field(Field::Body)?;
    let mut found: Option<String> = None;
    let mut ok = true;
    common::walk(body, |n| match kind(n) {
        "lambda_expression" | "class_body" => Walk::Skip,
        "return_statement" => {
            // Compared as written (whitespace dropped): the same text in one method names the
            // same type, including types of libraries the resolver does not know.
            let t: Option<String> = syntax::first_named_child(n)
                .and_then(|e| declared_type_node(ctx, e))
                .map(|t| ctx.text(t))
                .filter(|t| t.contains('<') && !t.contains('?'))
                .map(|t| t.split_whitespace().collect());
            match t {
                Some(t) if found.as_ref().is_none_or(|f| *f == t) => {
                    found = Some(t);
                    Walk::Skip
                }
                _ => {
                    ok = false;
                    Walk::Stop
                }
            }
        }
        _ => Walk::Continue,
    });
    if ok {
        found
    } else {
        None
    }
}

impl<'t> JavaRule<'t> for WildcardReturnType<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "method_declaration",
            "type_parameter",
            "class_declaration",
            "enum_declaration",
            "record_declaration",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        match kind(node) {
            "method_declaration" => Self::check_method(ctx, node),
            "type_parameter" => {
                let Some(bound) = syntax::child_of_kind(node, "type_bound") else {
                    return;
                };
                let mut types = named_children(bound);
                if let (Some(t), None) = (types.next(), types.next()) {
                    self.params.push((node, t));
                }
            }
            _ => {
                let is_final =
                    kind(node) != "class_declaration" || syntax::has_modifier(node, "final");
                if is_final {
                    if let Some(fqn) = ctx.class_fqn(node) {
                        self.final_types.push(fqn);
                    }
                }
            }
        }
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        for (param, bound) in &self.params {
            let Some(t) = ctx.resolve_type(*bound) else {
                continue;
            };
            let Some(fqn) = t.fqn() else {
                continue;
            };
            let is_final = resolve::is_final_class(fqn)
                || resolve::is_enum_class(fqn)
                || self.final_types.iter().any(|f| f == fqn);
            if !is_final {
                continue;
            }
            let name = syntax::first_named_child(*param)
                .map(|n| ctx.text(n))
                .unwrap_or("T");
            let b = ctx.text(*bound);
            ctx.emit(
                *param,
                format!(
                    "Replace the type parameter `{name}` by `{b}`: `{b}` cannot be extended, so `{name}` can only be `{b}`"
                ),
            );
        }
    }
}
