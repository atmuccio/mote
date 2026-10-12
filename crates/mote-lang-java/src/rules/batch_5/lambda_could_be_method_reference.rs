//! `java/lambda-could-be-method-reference` — `x -> foo(x)` is `this::foo`.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["idiom", "readability"],
    remediation_minutes: 1,
    description: r#"Detects lambdas whose body does nothing but pass the lambda's parameters, in
order, to one method or constructor: `x -> service.handle(x)`, `(a, b) -> merge(a, b)`,
`s -> s.trim()`, `name -> new Label(name)`, `() -> list.clear()`.

**Why it matters.** A method reference names the operation directly (`service::handle`,
`String::trim`, `Label::new`) without inventing parameter names that carry no meaning. It is
shorter and reads as "what", not "how".

**How to fix.** Replace the lambda with the corresponding method reference.

```java
// Noncompliant
names.forEach(n -> System.out.println(n));
orders.stream().map(o -> o.getTotal()).forEach(t -> ledger.add(t));

// Compliant
names.forEach(System.out::println);
orders.stream().map(Order::getTotal).forEach(ledger::add);
```

Only receivers that a method reference evaluates the same way are accepted: `this`, `super`,
a type name, a local variable or parameter, a `final` field, `System.out` / `System.err`, or
the lambda's first parameter. Receivers computed by a call (`() -> getService().run()`) are
evaluated once by a method reference but at every call by the lambda, so they are left alone,
as are generic calls with explicit type arguments and names that are ambiguous as unbound
references (`toString`, `hashCode`, …)."#,
    ..RuleMeta::new(
        "java/lambda-could-be-method-reference",
        "java",
        "Lambda could be a method reference",
    )
};

/// Methods that exist both as static and instance methods on common classes (`Integer`,
/// `Double`, …): a reference like `Integer::toString` is ambiguous.
const AMBIGUOUS: &[&str] = &[
    "toString",
    "hashCode",
    "isNaN",
    "isInfinite",
    "isFinite",
    "compare",
];

pub struct LambdaCouldBeMethodReference;

/// The lambda's parameter names (at most 8); `None` for unusual shapes.
fn params<'t>(ctx: &Ctx<'t>, lambda: Node<'t>) -> Option<Vec<&'t str>> {
    let p = util::fld(lambda, F.parameters)?;
    let mut out = Vec::new();
    match kind(p) {
        "identifier" => out.push(ctx.text(p)),
        "inferred_parameters" => {
            for c in named_children(p) {
                if kind(c) != "identifier" {
                    return None;
                }
                out.push(ctx.text(c));
            }
        }
        "formal_parameters" => {
            for c in named_children(p) {
                if kind(c) != "formal_parameter" {
                    return None;
                }
                out.push(ctx.text(util::fld(c, F.name)?));
            }
        }
        _ => return None,
    }
    (out.len() <= 8).then_some(out)
}

/// Whether the arguments of `call` are exactly `names`, in order.
fn args_are<'t>(ctx: &Ctx<'t>, call: Node<'t>, names: &[&'t str]) -> bool {
    let mut it = util::args(call);
    for n in names {
        match it.next() {
            Some(a) if kind(a) == "identifier" && ctx.text(a) == *n => {}
            _ => return false,
        }
    }
    it.next().is_none()
}

/// The expression the lambda evaluates (expression body, or a block with one
/// `return e;` / `e;`).
fn body_expr<'t>(lambda: Node<'t>) -> Option<Node<'t>> {
    let body = util::fld(lambda, F.body)?;
    if kind(body) != "block" {
        return Some(syntax::strip_parens(body));
    }
    let mut it = named_children(body);
    let (Some(stmt), None) = (it.next(), it.next()) else {
        return None;
    };
    match kind(stmt) {
        "return_statement" | "expression_statement" => {
            syntax::first_named_child(stmt).map(syntax::strip_parens)
        }
        _ => None,
    }
}

impl LambdaCouldBeMethodReference {
    /// The method reference text when it can be written with certainty, else `None` (the
    /// finding is still reported, with a generic message).
    fn suggestion<'t>(ctx: &Ctx<'t>, call: Node<'t>, names: &[&'t str]) -> Option<Option<String>> {
        match kind(call) {
            "object_creation_expression" => {
                if util::anonymous_body(call).is_some() || !args_are(ctx, call, names) {
                    return None;
                }
                let ty = util::fld(call, F.ty)?;
                if named_children(call).any(|c| kind(c) == "type_arguments") {
                    return None;
                }
                let base = ctx.text(util::type_base(ty));
                Some(Some(format!("{base}::new")))
            }
            "method_invocation" => {
                if util::fld(call, F.name).is_none()
                    || named_children(call).any(|c| kind(c) == "type_arguments")
                {
                    return None;
                }
                let name = util::call_name(ctx, call);
                match util::fld(call, F.object) {
                    None => args_are(ctx, call, names).then_some(None),
                    Some(obj) => {
                        let obj = syntax::strip_parens(obj);
                        match kind(obj) {
                            "this" | "super" => args_are(ctx, call, names)
                                .then(|| Some(format!("{}::{name}", ctx.text(obj)))),
                            "identifier" => {
                                let recv = ctx.text(obj);
                                if names.first() == Some(&recv) {
                                    // Unbound receiver: `s -> s.trim()` is `String::trim`.
                                    if AMBIGUOUS.contains(&name)
                                        || !args_are(ctx, call, &names[1..])
                                    {
                                        return None;
                                    }
                                    return Some(None);
                                }
                                if names.contains(&recv) || !args_are(ctx, call, names) {
                                    return None;
                                }
                                if let Some(d) = ctx.declaration_of(obj) {
                                    let ok = match d.kind {
                                        VarKind::Local
                                        | VarKind::Parameter
                                        | VarKind::CatchParameter
                                        | VarKind::ForEachVariable
                                        | VarKind::Resource
                                        | VarKind::PatternVariable => true,
                                        VarKind::Field => util::Mods::of(d.statement).has("final"),
                                        _ => false,
                                    };
                                    return ok.then(|| Some(format!("{recv}::{name}")));
                                }
                                // A type name: `x -> Integer.parseInt(x)`.
                                if AMBIGUOUS.contains(&name) || ctx.type_name_of(obj).is_none() {
                                    return None;
                                }
                                Some(Some(format!("{recv}::{name}")))
                            }
                            "field_access" => {
                                let t = ctx.text(obj);
                                if !matches!(t, "System.out" | "System.err")
                                    || !args_are(ctx, call, names)
                                {
                                    return None;
                                }
                                Some(Some(format!("{t}::{name}")))
                            }
                            _ => None,
                        }
                    }
                }
            }
            _ => None,
        }
    }
}

impl<'t> JavaRule<'t> for LambdaCouldBeMethodReference {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["lambda_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(call) = body_expr(node) else {
            return;
        };
        if !matches!(
            kind(call),
            "method_invocation" | "object_creation_expression"
        ) {
            return;
        }
        let Some(names) = params(ctx, node) else {
            return;
        };
        let Some(suggestion) = Self::suggestion(ctx, call, &names) else {
            return;
        };
        let msg = match suggestion {
            Some(s) => format!("Replace this lambda with the method reference `{s}`"),
            None => "Replace this lambda with a method reference".to_string(),
        };
        ctx.emit(node, msg);
    }
}
