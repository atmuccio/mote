//! `java/overload-instead-of-override` — a subclass method that looks like an override of a
//! superclass method but overloads it, because a parameter type differs only by boxing or by
//! package.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["inheritance", "suspicious"],
    remediation_minutes: 5,
    description: r#"Detects methods of a subclass that have the name and the number of parameters of
a superclass method, but where a parameter type differs only by boxing (`int` versus
`Integer`) or only by package (two different classes with the same simple name, such as
`java.util.Date` and `java.sql.Date`). The superclass must be declared in the same file.

**Why it matters.** Such a method does not override: it adds an overload. Code that calls the
method through a reference of the superclass type still runs the superclass version, so the
subclass's behaviour is silently skipped. Without `@Override` the compiler does not
complain, and the near-identical signature hides the difference from readers.

**How to fix.** Use exactly the superclass parameter types (and add `@Override`, so that the
compiler checks it), or give the method a different name if an overload is really meant.

```java
// Noncompliant
class Handler {
    void handle(Integer code) { … }
}
class RetryHandler extends Handler {
    void handle(int code) { … }       // never called through a Handler reference
}

// Compliant
class RetryHandler extends Handler {
    @Override
    void handle(Integer code) { … }
}
```

Methods annotated `@Override` (the compiler already checks them), static and private
methods, methods of the superclass using its type parameters, and superclasses declared in
other files are not considered."#,
    ..RuleMeta::new(
        "java/overload-instead-of-override",
        "java",
        "Overload that looks like an override",
    )
};

/// Type declarations are collected during the walk and matched with superclasses in `finish`
/// (a superclass may be declared after its subclass).
#[derive(Default)]
pub struct OverloadInsteadOfOverride<'t> {
    types: Vec<Node<'t>>,
}

fn boxed_name(p: &str) -> Option<&'static str> {
    Some(match p {
        "int" => "Integer",
        "long" => "Long",
        "short" => "Short",
        "byte" => "Byte",
        "char" => "Character",
        "boolean" => "Boolean",
        "double" => "Double",
        "float" => "Float",
        _ => return None,
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Diff {
    Same,
    /// Differs only by boxing or by package: looks like the same type.
    LookAlike,
    /// A genuinely different type.
    Other,
}

fn compare_types<'t>(ctx: &Ctx<'t>, a: Node<'t>, b: Node<'t>) -> Diff {
    let (ta, tb) = (ctx.text(a), ctx.text(b));
    if ta == tb {
        // Same text; different imports are impossible within one file.
        return Diff::Same;
    }
    if ta.contains('<') || tb.contains('<') || ta.contains('[') || tb.contains('[') {
        return Diff::Other;
    }
    let simple = |t: &'t str| t.rsplit('.').next().unwrap_or(t);
    if boxed_name(ta) == Some(simple(tb)) || boxed_name(tb) == Some(simple(ta)) {
        return Diff::LookAlike;
    }
    if simple(ta) == simple(tb) {
        let fa = ctx.resolve_type_name(ta, a);
        let fb = ctx.resolve_type_name(tb, b);
        return match (fa, fb) {
            (Some(x), Some(y)) if x != y => Diff::LookAlike,
            (Some(_), Some(_)) => Diff::Same,
            _ => Diff::Other,
        };
    }
    Diff::Other
}

fn param_types(m: Node<'_>) -> Option<Vec<Node<'_>>> {
    syntax::parameters(m)
        .map(|p| {
            if kind(p) == "spread_parameter" {
                None
            } else {
                p.field(Field::Type)
            }
        })
        .collect()
}

impl<'t> JavaRule<'t> for OverloadInsteadOfOverride<'t> {
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

    fn check(&mut self, node: Node<'t>, _ctx: &Ctx<'t>) {
        self.types.push(node);
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        let src = ctx.source();
        // Type names declared once in the file, with their declaration.
        let mut by_name: FxHashMap<&str, Option<Node<'t>>> = FxHashMap::default();
        for &t in &self.types {
            by_name
                .entry(syntax::name_of(t, src))
                .and_modify(|e| *e = None)
                .or_insert(Some(t));
        }
        for &node in &self.types {
            if kind(node) != "class_declaration" {
                continue;
            }
            let Some(sup) = node.field(Field::Superclass) else {
                continue;
            };
            let Some(ty) = syntax::first_named_child(sup) else {
                continue;
            };
            if kind(ty) != "type_identifier" {
                continue;
            }
            let Some(Some(parent)) = by_name.get(ctx.text(ty)).copied() else {
                continue;
            };
            check_class(ctx, node, parent);
        }
    }
}

fn check_class<'t>(ctx: &Ctx<'t>, node: Node<'t>, parent: Node<'t>) {
    if kind(parent) != "class_declaration"
        || parent.field(Field::TypeParameters).is_some()
        || parent.id() == node.id()
    {
        return;
    }
    let (Some(body), Some(pbody)) = (syntax::type_body(node), syntax::type_body(parent)) else {
        return;
    };
    let src = ctx.source();
    let parent_methods: Vec<Node<'t>> = ctx
        .members(pbody)
        .filter(|m| {
            kind(*m) == "method_declaration"
                && !syntax::has_modifier(*m, "private")
                && !syntax::has_modifier(*m, "static")
        })
        .collect();
    if parent_methods.is_empty() {
        return;
    }
    for m in ctx.members(body) {
        if kind(m) != "method_declaration"
            || syntax::has_modifier(m, "static")
            || syntax::has_modifier(m, "private")
            || syntax::has_annotation(m, src, "Override")
        {
            continue;
        }
        let name = syntax::name_of(m, src);
        let Some(mine) = param_types(m) else { continue };
        if mine.is_empty() {
            continue;
        }
        let mut look_alike = None;
        let mut overrides = false;
        for p in parent_methods
            .iter()
            .filter(|p| syntax::name_of(**p, src) == name)
        {
            let Some(theirs) = param_types(*p) else {
                overrides = true;
                break;
            };
            if theirs.len() != mine.len() {
                continue;
            }
            let diffs: Vec<Diff> = mine
                .iter()
                .zip(&theirs)
                .map(|(a, b)| compare_types(ctx, *a, *b))
                .collect();
            if diffs.iter().all(|d| *d == Diff::Same) {
                overrides = true;
                break;
            }
            if diffs.iter().all(|d| *d != Diff::Other) {
                look_alike = Some(*p);
            }
        }
        if overrides {
            continue;
        }
        let Some(p) = look_alike else { continue };
        let (Some(at), Some(pat)) = (m.field(Field::Name), p.field(Field::Name)) else {
            continue;
        };
        ctx.emit(
                at,
                format!(
                    "`{name}` overloads `{}.{name}` instead of overriding it: a parameter type differs only by boxing or package",
                    syntax::name_of(parent, src)
                ),
            )
            .secondary(pat, "superclass method");
    }
}
