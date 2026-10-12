//! `java/nullability-annotation-misuse` — nullability annotations that cannot mean anything:
//! on primitive types, and a non-null marker on the parameter of `equals(Object)`.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Incubating,
    tags: &["null", "annotations", "confusing"],
    remediation_minutes: 2,
    description: r#"Detects nullability annotations that say something impossible or wrong:

* `@Nullable`, `@NonNull`, `@NotNull`, `@CheckForNull` (and their spellings in the common
  annotation libraries) on a field, parameter, local variable, record component or method
  return of a primitive type (`int`, `long`, `boolean`, …) or `void`;
* a non-null marker (`@NonNull`, `@Nonnull`, `@NotNull`) on the parameter of
  `equals(Object)`;
* an override that turns a `@Nullable` parameter of the method it overrides into a non-null
  one, or a non-null return into a `@Nullable` one, when the overridden method is declared in
  the same file.

**Why it matters.** A primitive can never be `null`: the annotation is noise at best, and at
worst tells the reader (or a null checker) that `null` is a possible value when it is not —
often the leftover of a change from `Integer` to `int`. The `equals` contract requires
`x.equals(null)` to return `false`, so a non-null parameter promises callers something the
method must not demand. An override may accept more than its parent and return less, never
the reverse: callers that follow the parent's contract would pass `null` where the override
rejects it, or get `null` where the parent promised a value.

**How to fix.** Remove the annotation from primitives (or go back to the boxed type if `null`
really is a state), drop the non-null marker from `equals`, and keep an override's contract at
least as permissive as its parent's.

```java
// Noncompliant
@Nullable int retries;
public boolean equals(@NonNull Object other) { … }

// Compliant
int retries;
public boolean equals(@Nullable Object other) { … }
```

Not reported: arrays of primitives (a declaration annotation on `int[]` describes the array,
which can be `null`), boxed types, overrides of methods declared in other files (their
contract is not visible here), and test code (tests of validation and null-checking tools
use such declarations on purpose)."#,
    ..RuleMeta::new(
        "java/nullability-annotation-misuse",
        "java",
        "Nullability annotation that cannot apply",
    )
};

#[derive(Default)]
pub struct NullabilityAnnotationMisuse<'t> {
    /// Whether the file mentions a nullability annotation at all (computed on first use).
    relevant: Option<bool>,
    /// Classes and interfaces of this file: (declaration, simple name).
    types: Vec<(Node<'t>, &'t str)>,
    /// Methods with `@Override` and at least one nullability annotation: (method, class).
    overrides: Vec<(Node<'t>, Node<'t>)>,
}

/// Annotations saying "may be null".
const NULLABLE: &[&str] = &[
    "Nullable",
    "CheckForNull",
    "NullableDecl",
    "NullAllowed",
    "PolyNull",
];

/// Annotations saying "never null".
const NON_NULL: &[&str] = &[
    "NonNull",
    "Nonnull",
    "NotNull",
    "NonNullDecl",
    "MonotonicNonNull",
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Nullness {
    Nullable,
    NonNull,
}

fn nullness(name: &str) -> Option<Nullness> {
    if NULLABLE.contains(&name) {
        Some(Nullness::Nullable)
    } else if NON_NULL.contains(&name) {
        Some(Nullness::NonNull)
    } else {
        None
    }
}

/// The first nullability annotation of a declaration and what it says.
fn nullability_annotation<'t>(
    ctx: &Ctx<'t>,
    decl: Node<'t>,
) -> Option<(Node<'t>, &'t str, Nullness)> {
    util::annotation_names(ctx, decl).find_map(|(a, name)| nullness(name).map(|n| (a, name, n)))
}

fn is_primitive_type(t: Node<'_>) -> bool {
    matches!(
        kind(t),
        "integral_type" | "floating_point_type" | "boolean_type" | "void_type"
    )
}

impl<'t> JavaRule<'t> for NullabilityAnnotationMisuse<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "formal_parameter",
            "method_declaration",
            "field_declaration",
            "local_variable_declaration",
            "class_declaration",
            "interface_declaration",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let relevant = *self.relevant.get_or_insert_with(|| {
            let src = ctx.source();
            [
                "Nullable",
                "NonNull",
                "Nonnull",
                "NotNull",
                "CheckForNull",
                "NullAllowed",
                "PolyNull",
            ]
            .iter()
            .any(|a| src.contains(a))
        });
        if !relevant || ctx.is_test() {
            return;
        }
        match kind(node) {
            "class_declaration" | "interface_declaration" => {
                self.types.push((node, ctx.name_of(node)));
                return;
            }
            "method_declaration" => self.check_method(node, ctx),
            _ => {}
        }
        let Some(ty) = node.field(Field::Type) else {
            return;
        };
        if !is_primitive_type(ty) || syntax::modifiers(node).is_none() {
            return;
        }
        // `int x[]` and `int f()[]` are arrays.
        let array = match kind(node) {
            "formal_parameter" | "method_declaration" => node.field(Field::Dimensions).is_some(),
            _ => syntax::declarators(node).any(|d| d.field(Field::Dimensions).is_some()),
        };
        if array {
            return;
        }
        let Some((ann, name, n)) = nullability_annotation(ctx, node) else {
            return;
        };
        let ty_text = ctx.text(ty);
        let msg = if ty_text == "void" {
            format!("Remove `@{name}`: a `void` method returns no value")
        } else if n == Nullness::Nullable {
            format!("Remove `@{name}`: `{ty_text}` values can never be null")
        } else {
            format!("Remove `@{name}`: `{ty_text}` values are never null anyway")
        };
        ctx.emit(ann, msg);
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        for &(m, class) in &self.overrides {
            self.check_override(ctx, m, class);
        }
    }
}

impl<'t> NullabilityAnnotationMisuse<'t> {
    fn check_method(&mut self, m: Node<'t>, ctx: &Ctx<'t>) {
        let src = ctx.source();
        let name = syntax::name_of(m, src);
        if name == "equals" {
            let mut params = syntax::parameters(m);
            if let (Some(p), None) = (params.next(), params.next()) {
                let is_object = p
                    .field(Field::Type)
                    .is_some_and(|t| matches!(ctx.text(t), "Object" | "java.lang.Object"));
                if is_object && kind(p) == "formal_parameter" {
                    if let Some((ann, aname, Nullness::NonNull)) = nullability_annotation(ctx, p) {
                        ctx.emit(
                            ann,
                            format!("Remove `@{aname}` from the parameter of `equals`: its contract accepts `null` (and answers `false`)"),
                        );
                        return;
                    }
                }
            }
        }
        if !syntax::has_annotation(m, src, "Override") {
            return;
        }
        let annotated = nullability_annotation(ctx, m).is_some()
            || syntax::parameters(m).any(|p| nullability_annotation(ctx, p).is_some());
        if !annotated {
            return;
        }
        if let Some(class) = ctx.enclosing_class(m) {
            if kind(class) == "class_declaration" || kind(class) == "interface_declaration" {
                self.overrides.push((m, class));
            }
        }
    }

    /// Compares an override with the same-file method it overrides (direct supertypes only).
    fn check_override(&self, ctx: &Ctx<'t>, m: Node<'t>, class: Node<'t>) {
        let src = ctx.source();
        let name = syntax::name_of(m, src);
        let params = param_types(ctx, m);
        for sup in self.same_file_supertypes(ctx, class) {
            let Some(body) = sup.field(Field::Body) else {
                continue;
            };
            let Some(parent) = ctx.members(body).find(|pm| {
                kind(*pm) == "method_declaration"
                    && syntax::name_of(*pm, src) == name
                    && !syntax::has_modifier(*pm, "private")
                    && !syntax::has_modifier(*pm, "static")
                    && param_types(ctx, *pm) == params
            }) else {
                continue;
            };
            let sup_name = syntax::name_of(sup, src);
            // Parameters: nullable in the parent, non-null in the override.
            for (p, pp) in syntax::parameters(m).zip(syntax::parameters(parent)) {
                if let (Some((ann, _, Nullness::NonNull)), Some((_, _, Nullness::Nullable))) = (
                    nullability_annotation(ctx, p),
                    nullability_annotation(ctx, pp),
                ) {
                    ctx.emit(
                        ann,
                        format!("Accept `null` here as `{sup_name}.{name}` does: an override must not narrow what its parent accepts"),
                    )
                    .secondary(pp, "declared nullable here");
                }
            }
            // Return: non-null in the parent, nullable in the override.
            if let (Some((ann, _, Nullness::Nullable)), Some((pann, _, Nullness::NonNull))) = (
                nullability_annotation(ctx, m),
                nullability_annotation(ctx, parent),
            ) {
                ctx.emit(
                    ann,
                    format!("Do not return `null` from this override: `{sup_name}.{name}` promises a non-null result"),
                )
                .secondary(pann, "declared non-null here");
            }
            return;
        }
    }

    /// The direct superclass and interfaces of `class` that are declared in this file.
    fn same_file_supertypes(&self, ctx: &Ctx<'t>, class: Node<'t>) -> Vec<Node<'t>> {
        let mut out = Vec::new();
        let mut consider = |ty: Node<'t>| {
            let base = if kind(ty) == "generic_type" {
                match syntax::first_named_child(ty) {
                    Some(b) => b,
                    None => return,
                }
            } else {
                ty
            };
            let name = util::last_segment(ctx.text(base));
            let Some(&(decl, _)) = self.types.iter().find(|(_, n)| *n == name) else {
                return;
            };
            let (Some(fqn), Some(resolved)) = (ctx.class_fqn(decl), ctx.resolve_type(base)) else {
                return;
            };
            if resolved.fqn() == Some(fqn.as_str()) {
                out.push(decl);
            }
        };
        if let Some(sc) = class
            .field(Field::Superclass)
            .and_then(syntax::first_named_child)
        {
            consider(sc);
        }
        let lists = class
            .field(Field::Interfaces)
            .into_iter()
            .chain(syntax::child_of_kind(class, "extends_interfaces"));
        for list in lists {
            for tl in named_children(list) {
                if kind(tl) == "type_list" {
                    named_children(tl).for_each(&mut consider);
                } else {
                    consider(tl);
                }
            }
        }
        out
    }
}

/// The parameter types of a method as written.
fn param_types<'t>(ctx: &Ctx<'t>, m: Node<'t>) -> Vec<&'t str> {
    syntax::parameters(m)
        .map(|p| {
            p.field(Field::Type)
                .or_else(|| syntax::first_named_child(p))
                .map(|t| ctx.text(t))
                .unwrap_or("")
        })
        .collect()
}
