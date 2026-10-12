//! `java/equals-without-hashcode` — a declaration-level rule (looks at one class body).

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["equality", "collections"],
    remediation_minutes: 15,
    description: r#"Detects classes that override `equals(Object)` without overriding
`hashCode()`, or the other way round.

**Why it matters.** Hash-based collections (`HashMap`, `HashSet`, caches) require that equal
objects have equal hash codes. With only `equals` overridden, two equal objects usually land in
different buckets: lookups miss, duplicates appear in sets. With only `hashCode` overridden,
the class still compares by identity, which is almost never what the author intended.

**How to fix.** Override both, based on the same fields (`Objects.hash(...)` helps), or use a
`record`, which generates consistent implementations.

```java
// Noncompliant
class Point {
    final int x, y;
    @Override public boolean equals(Object o) { ... }
}

// Compliant
class Point {
    final int x, y;
    @Override public boolean equals(Object o) { ... }
    @Override public int hashCode() { return Objects.hash(x, y); }
}
```

Records are not reported, nor an `equals` that only returns `super.equals(o)` (it keeps the
superclass's equality, which matches the inherited `hashCode`), an identity `equals`
(`return this == o;`, which every `hashCode` is consistent with), or a `hashCode` that only
returns `super.hashCode()`. Inherited methods are taken into account where they may matter: a
`hashCode` without `equals` in a class that extends another is only reported when every
superclass is declared in this file and none of them declares `equals` (an inherited `equals`
may compare the same fields), and an `equals` without `hashCode` is not reported when a
superclass of this file redeclares `hashCode()` abstract, or when the class is abstract and
extends a class of another file (which may do so). Lombok's `@EqualsAndHashCode`, `@Data` and
`@Value` do not help here: they generate both methods or neither, and neither as soon as the
class writes one of them, so a hand-written `equals` in such a class still lacks its
`hashCode`."#,
    ..RuleMeta::new(
        "java/equals-without-hashcode",
        "java",
        "equals() and hashCode() not overridden together",
    )
};

pub struct EqualsWithoutHashCode;

/// Whether `method` is `equals(Object)` or `hashCode()` (by name and parameters), and not
/// static.
fn is_member<'t>(ctx: &Ctx<'t>, method: Node<'t>, name: &str) -> bool {
    if kind(method) != "method_declaration"
        || method
            .field(Field::Name)
            .is_none_or(|n| ctx.text(n) != name)
        || syntax::has_modifier(method, "static")
    {
        return false;
    }
    let mut params = syntax::parameters(method);
    match name {
        "equals" => match (params.next(), params.next()) {
            (Some(p), None) => {
                p.field(Field::Type)
                    .is_some_and(|t| matches!(ctx.text(t), "Object" | "java.lang.Object"))
                    && p.field(Field::Dimensions).is_none()
            }
            _ => false,
        },
        _ => params.next().is_none(),
    }
}

/// The expression of a body that is a single `return <expr>;` (parentheses stripped).
fn single_returned(method: Node<'_>) -> Option<Node<'_>> {
    let body = method.field(Field::Body)?;
    let mut stmts = named_children(body);
    let (Some(ret), None) = (stmts.next(), stmts.next()) else {
        return None;
    };
    if kind(ret) != "return_statement" {
        return None;
    }
    syntax::first_named_child(ret).map(syntax::strip_parens)
}

/// `equals(Object o) { return super.equals(o); }` / `hashCode() { return super.hashCode(); }`:
/// the superclass's behaviour, consistent with the other inherited method.
fn delegates_to_super<'t>(ctx: &Ctx<'t>, method: Node<'t>, name: &str) -> bool {
    single_returned(method).is_some_and(|call| {
        kind(call) == "method_invocation"
            && ctx.call_name(call) == Some(name)
            && call
                .field(Field::Object)
                .is_some_and(|o| kind(o) == "super")
    })
}

/// `equals(Object o) { return this == o; }`: identity, which every `hashCode` is consistent
/// with.
fn is_identity_equals<'t>(ctx: &Ctx<'t>, method: Node<'t>) -> bool {
    let Some(param) = syntax::parameters(method)
        .next()
        .and_then(|p| p.field(Field::Name))
    else {
        return false;
    };
    let param = ctx.text(param);
    single_returned(method).is_some_and(|e| {
        if kind(e) != "binary_expression" || syntax::operator_str(e) != "==" {
            return false;
        }
        let (Some(l), Some(r)) = (e.field(Field::Left), e.field(Field::Right)) else {
            return false;
        };
        let (l, r) = (syntax::strip_parens(l), syntax::strip_parens(r));
        let is_param = |n: Node<'t>| kind(n) == "identifier" && ctx.text(n) == param;
        (kind(l) == "this" && is_param(r)) || (is_param(l) && kind(r) == "this")
    })
}

/// The type declaration of this file (top-level or member type, any depth) with the simple
/// name `name`, when there is exactly one.
fn find_type_decl<'t>(ctx: &Ctx<'t>, name: &str) -> Option<Node<'t>> {
    let mut found = None;
    let mut stack: Vec<Node<'t>> = named_children(ctx.root())
        .filter(|c| syntax::is_type_declaration(*c))
        .collect();
    let mut guard = 0;
    while let Some(decl) = stack.pop() {
        guard += 1;
        if guard > 10_000 {
            return None;
        }
        if syntax::name_of(decl, ctx.source()) == name {
            if found.is_some() {
                return None;
            }
            found = Some(decl);
        }
        if let Some(body) = syntax::type_body(decl) {
            stack.extend(
                ctx.members(body)
                    .filter(|m| syntax::is_type_declaration(*m)),
            );
        }
    }
    found
}

/// The superclasses of `class` declared in this file, nearest first, and whether the chain is
/// complete (ends at a class without `extends`, i.e. at `Object`). A superclass declared
/// elsewhere (a library, another file) leaves the chain incomplete.
fn in_file_superclasses<'t>(ctx: &Ctx<'t>, class: Node<'t>) -> (Vec<Node<'t>>, bool) {
    let mut chain: Vec<Node<'t>> = Vec::new();
    let mut cur = class;
    for _ in 0..16 {
        let Some(sup) = cur.field(Field::Superclass) else {
            return (chain, true);
        };
        let Some(ty) = syntax::first_named_child(sup) else {
            return (chain, false);
        };
        let ty = if kind(ty) == "generic_type" {
            syntax::first_named_child(ty).unwrap_or(ty)
        } else {
            ty
        };
        let text = ctx.text(ty);
        let simple = text.rsplit('.').next().unwrap_or(text).trim();
        match find_type_decl(ctx, simple) {
            Some(d) if kind(d) == "class_declaration" && d != class && !chain.contains(&d) => {
                chain.push(d);
                cur = d;
            }
            _ => return (chain, false),
        }
    }
    (chain, false)
}

/// Whether one of `classes` declares `name` (`equals(Object)` / `hashCode()`); with
/// `abstract_only`, only a bodiless (abstract) declaration counts.
fn declares<'t>(ctx: &Ctx<'t>, classes: &[Node<'t>], name: &str, abstract_only: bool) -> bool {
    classes.iter().any(|c| {
        syntax::type_body(*c).is_some_and(|body| {
            ctx.members(body).any(|m| {
                is_member(ctx, m, name) && (!abstract_only || m.field(Field::Body).is_none())
            })
        })
    })
}

impl<'t> JavaRule<'t> for EqualsWithoutHashCode {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["class_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        let mut equals = None;
        let mut hash_code = None;
        for m in ctx
            .members(body)
            .filter(|m| kind(*m) == "method_declaration")
        {
            // The name first: it rejects almost every method.
            let Some(name) = m.field(Field::Name) else {
                continue;
            };
            match ctx.text(name) {
                "equals" if is_member(ctx, m, "equals") => {
                    let harmless =
                        delegates_to_super(ctx, m, "equals") || is_identity_equals(ctx, m);
                    equals = Some((name, harmless));
                }
                "hashCode" if is_member(ctx, m, "hashCode") => {
                    hash_code = Some((name, delegates_to_super(ctx, m, "hashCode")));
                }
                _ => {}
            }
        }
        match (equals, hash_code) {
            (Some((eq, false)), None) => {
                // A superclass that redeclares `hashCode()` abstract forces every concrete
                // subclass to write one; an abstract class extending a class of another file
                // may be in that situation.
                if node.field(Field::Superclass).is_some() {
                    let (supers, complete) = in_file_superclasses(ctx, node);
                    if declares(ctx, &supers, "hashCode", true)
                        || (!complete && syntax::has_modifier(node, "abstract"))
                    {
                        return;
                    }
                }
                ctx.emit(
                    eq,
                    "Override hashCode() as well: equal objects must have equal hash codes",
                );
            }
            (None, Some((hc, false))) => {
                // With a superclass, `equals` may be inherited from it (and compare the same
                // fields): only report when every superclass is in this file and none
                // declares `equals`.
                if node.field(Field::Superclass).is_some() {
                    let (supers, complete) = in_file_superclasses(ctx, node);
                    if !complete || declares(ctx, &supers, "equals", false) {
                        return;
                    }
                }
                ctx.emit(
                    hc,
                    "Override equals(Object) as well, consistently with this hashCode()",
                );
            }
            _ => {}
        }
    }
}
