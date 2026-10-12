//! `java/identical-catch-branches` — catch clauses of one `try` with the same body.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["duplication", "exceptions"],
    remediation_minutes: 5,
    description: r#"Detects `catch` clauses of the same `try` statement whose bodies are
identical (the exception variable may have a different name in each).

**Why it matters.** Duplicated handlers have to be kept in sync by hand; sooner or later one
of them is changed and the other isn't. Since Java 7 a single multi-catch clause expresses
"handle these the same way" directly.

**How to fix.** Merge the clauses into one multi-catch.

```java
// Noncompliant
try {
    load(file);
} catch (IOException e) {
    log.warn("cannot load " + file, e);
} catch (ParseException e) {
    log.warn("cannot load " + file, e);
}

// Compliant
try {
    load(file);
} catch (IOException | ParseException e) {
    log.warn("cannot load " + file, e);
}
```

Reported only when merging is possible: the caught types must be provably unrelated (a
multi-catch may not list a subclass together with its superclass), so clauses catching types
unknown to the resolver are left alone. Neither are handlers that pass the caught exception to
a method or constructor whose overload may depend on its type (`throw wrap(ex);` with
`wrap(TimeoutException)` and `wrap(ExecutionException)`): the textually identical clauses
call different methods, and a multi-catch would call the most general one. That is assumed
for overloads declared in this file and for unqualified calls to methods this file does not
declare (inherited or statically imported). Empty handlers are not reported (see
`java/empty-catch-block`)."#,
    ..RuleMeta::new(
        "java/identical-catch-branches",
        "java",
        "Identical catch branches",
    )
};

pub struct IdenticalCatchBranches;

/// The caught type nodes of a catch clause (several for a multi-catch).
fn caught_types<'t>(clause: Node<'t>) -> Vec<Node<'t>> {
    named_children(clause)
        .find(|c| kind(*c) == "catch_formal_parameter")
        .and_then(|p| named_children(p).find(|c| kind(*c) == "catch_type"))
        .map(|t| named_children(t).collect())
        .unwrap_or_default()
}

/// Statement count and the kinds of the first and last statements of a block (comments
/// skipped).
fn shape(block: Node<'_>) -> (usize, &'static str, &'static str) {
    let mut n = 0;
    let (mut first, mut last) = ("", "");
    for s in named_children(block) {
        if n == 0 {
            first = kind(s);
        }
        last = kind(s);
        n += 1;
    }
    (n, first, last)
}

fn param_name<'t>(ctx: &Ctx<'t>, clause: Node<'t>) -> &'t str {
    named_children(clause)
        .find(|c| kind(*c) == "catch_formal_parameter")
        .and_then(|p| util::fld(p, F.name))
        .map_or("", |n| ctx.text(n))
}

/// Whether every type of `a` is provably unrelated to every type of `b`.
fn unrelated<'t>(ctx: &Ctx<'t>, a: &[Node<'t>], b: &[Node<'t>]) -> bool {
    for x in a {
        let Some(tx) = ctx.resolve_type(*x) else {
            return false;
        };
        for y in b {
            let Some(ty) = ctx.resolve_type(*y) else {
                return false;
            };
            let (Some(fx), Some(fy)) = (tx.fqn(), ty.fqn()) else {
                return false;
            };
            if ctx.subtype(&tx, fy) != Some(false) || ctx.subtype(&ty, fx) != Some(false) {
                return false;
            }
        }
    }
    true
}

/// Whether the handler passes the caught variable `var` directly as an argument to a call
/// whose overload may depend on its static type: a method or constructor with several
/// overloads of that arity in this file, or an unqualified call to a method this file does not
/// declare (inherited or statically imported, overloads unknown).
fn type_dependent_call<'t>(ctx: &Ctx<'t>, body: Node<'t>, var: &str) -> bool {
    if !ctx.text(body).contains(var) {
        return false;
    }
    let mut decls: Option<Vec<Node<'t>>> = None;
    util::visit(body, |n| {
        let (name, unqualified) = match kind(n) {
            "method_invocation" => (
                util::call_name(ctx, n),
                util::fld(n, F.object).is_none_or(|o| matches!(kind(o), "this" | "super")),
            ),
            "object_creation_expression" => {
                let ty = util::fld(n, F.ty).map(util::type_base);
                let text = ty.map_or("", |t| ctx.text(t));
                (text.rsplit('.').next().unwrap_or(text), false)
            }
            _ => return util::Walk::Continue,
        };
        let Some(args) = util::fld(n, F.arguments) else {
            return util::Walk::Continue;
        };
        let passes = named_children(args).any(|a| {
            let a = syntax::strip_parens(a);
            kind(a) == "identifier" && ctx.text(a) == var
        });
        if !passes || name.is_empty() {
            return util::Walk::Continue;
        }
        let arity = named_children(args).count();
        let all = decls.get_or_insert_with(|| file_members(ctx));
        let is_ctor = kind(n) == "object_creation_expression";
        let same_name: Vec<&Node<'t>> = all
            .iter()
            .filter(|d| {
                if is_ctor {
                    kind(**d) == "constructor_declaration"
                } else {
                    kind(**d) == "method_declaration"
                }
            })
            .filter(|d| syntax::name_of(**d, ctx.source()) == name)
            .collect();
        let overloads = same_name
            .iter()
            .filter(|d| syntax::parameters(***d).count() == arity)
            .count();
        let dependent = overloads >= 2 || (unqualified && same_name.is_empty());
        if dependent {
            util::Walk::Stop
        } else {
            util::Walk::Continue
        }
    })
}

/// Methods and constructors of every type declared in this file (bodies not entered).
fn file_members<'t>(ctx: &Ctx<'t>) -> Vec<Node<'t>> {
    let mut out = Vec::new();
    let mut stack: Vec<Node<'t>> = named_children(ctx.root())
        .filter(|c| syntax::is_type_declaration(*c))
        .collect();
    let mut guard = 0;
    while let Some(decl) = stack.pop() {
        guard += 1;
        if guard > 10_000 {
            break;
        }
        let Some(body) = syntax::type_body(decl) else {
            continue;
        };
        for m in ctx.members(body) {
            match kind(m) {
                "method_declaration" | "constructor_declaration" => out.push(m),
                _ if syntax::is_type_declaration(m) => stack.push(m),
                _ => {}
            }
        }
    }
    out
}

impl<'t> JavaRule<'t> for IdenticalCatchBranches {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["try_statement", "try_with_resources_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let mut catches: Vec<Node<'t>> = Vec::new();
        for c in named_children(node) {
            if kind(c) == "catch_clause" {
                catches.push(c);
            }
        }
        if catches.len() < 2 {
            return;
        }
        let src = ctx.source();
        for i in 1..catches.len() {
            let Some(body_i) = util::fld(catches[i], F.body) else {
                continue;
            };
            if syntax::is_empty_block(body_i) {
                continue;
            }
            let name_i = param_name(ctx, catches[i]);
            for j in 0..i {
                let Some(body_j) = util::fld(catches[j], F.body) else {
                    continue;
                };
                let name_j = param_name(ctx, catches[j]);
                // Cheap shape test before comparing tokens.
                if shape(body_j) != shape(body_i)
                    || !util::same_tokens(body_j, body_i, src, (name_j, name_i))
                {
                    continue;
                }
                let (ti, tj) = (caught_types(catches[i]), caught_types(catches[j]));
                if ti.is_empty() || tj.is_empty() || !unrelated(ctx, &tj, &ti) {
                    continue;
                }
                if type_dependent_call(ctx, body_i, name_i) {
                    continue;
                }
                let first_type = tj.first().map_or("", |t| ctx.text(*t));
                let kw = syntax::children(catches[i]).next().unwrap_or(catches[i]);
                ctx.emit(
                    kw,
                    format!(
                        "Merge this catch clause with the identical one for `{first_type}` into a \
                         multi-catch"
                    ),
                )
                .secondary(catches[j], "identical handler");
                break;
            }
        }
    }
}
