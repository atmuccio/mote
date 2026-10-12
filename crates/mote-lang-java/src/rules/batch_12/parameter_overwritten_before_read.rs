//! `java/parameter-overwritten-before-read` — a parameter, caught exception or loop element
//! whose incoming value is replaced before anything reads it.

use super::util::any_descendant;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["suspicious", "unused"],
    remediation_minutes: 5,
    description: r#"Detects a method, constructor or lambda parameter, a `catch` parameter, or an
enhanced-`for` variable that is assigned a new value before its incoming value has been read.

**Why it matters.** The value the caller passed (the exception that was caught, the element of
the current iteration) is thrown away unseen. Either the parameter is useless and misleads
callers into thinking their argument matters, or, far more often, the assignment is a slip:
it was meant to go the other way (`this.name = name`), to target a field or another variable,
or to happen after the value was used.

**How to fix.** Use the incoming value, fix the target of the assignment, or remove the
parameter and declare a local variable instead.

```java
// Noncompliant
Account(String owner, int limit) {
    owner = "unknown";            // the caller's owner is ignored
    this.owner = owner;
    this.limit = limit;
}

// Compliant
Account(String owner, int limit) {
    this.owner = owner == null ? "unknown" : owner;
    this.limit = limit;
}
```

Only an assignment that is a statement of the body itself (not nested in a branch or loop)
and comes before any other mention of the name is reported; `final` parameters cannot be
reassigned and are skipped, as are values computed from the name itself (`s = s.trim()`)."#,
    ..RuleMeta::new(
        "java/parameter-overwritten-before-read",
        "java",
        "Parameter overwritten before it is read",
    )
};

pub struct ParameterOverwrittenBeforeRead;

impl<'t> JavaRule<'t> for ParameterOverwrittenBeforeRead {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["assignment_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        // `name = value;` as a statement directly in the body of the declaring construct.
        if syntax::operator_str(node) != "=" {
            return;
        }
        let Some(target) = node.field(Field::Left).map(syntax::strip_parens) else {
            return;
        };
        if kind(target) != "identifier" {
            return;
        }
        let mut anc = ctx.ancestors(node);
        let (Some(stmt), Some(body), Some(owner)) = (anc.next(), anc.next(), anc.next()) else {
            return;
        };
        if kind(stmt) != "expression_statement"
            || !matches!(kind(body), "block" | "constructor_body")
            || owner.field(Field::Body).is_none_or(|b| b.id() != body.id())
        {
            return;
        }
        let name = ctx.text(target);
        let Some(what) = declares(owner, name, ctx) else {
            return;
        };
        // Every statement before this one must leave the name alone.
        for s in named_children(body) {
            if s.id() == stmt.id() {
                break;
            }
            if mentions(s, name, ctx) {
                return;
            }
        }
        if node
            .field(Field::Right)
            .is_none_or(|v| mentions(v, name, ctx))
        {
            return;
        }
        ctx.emit(
            target,
            format!("Use or drop the {what} `{name}`; its incoming value is overwritten here before it is read"),
        );
    }
}

/// Whether `owner` (method, constructor, lambda, catch clause, enhanced `for`) declares a
/// non-`final` variable `name`, and what kind of variable it is.
fn declares<'t>(owner: Node<'t>, name: &str, ctx: &Ctx<'t>) -> Option<&'static str> {
    let named = |n: Option<Node<'t>>| n.is_some_and(|n| ctx.text(n) == name);
    match kind(owner) {
        "method_declaration" | "constructor_declaration" => syntax::parameters(owner)
            .any(|p| {
                !syntax::has_modifier(p, "final")
                    && match kind(p) {
                        "formal_parameter" => named(p.field(Field::Name)),
                        "spread_parameter" => named(
                            named_children(p)
                                .find(|c| kind(*c) == "variable_declarator")
                                .and_then(|d| d.field(Field::Name)),
                        ),
                        _ => false,
                    }
            })
            .then_some("parameter"),
        "lambda_expression" => {
            let params = owner.field(Field::Parameters)?;
            let found = match kind(params) {
                "identifier" => ctx.text(params) == name,
                "inferred_parameters" => {
                    named_children(params).any(|p| kind(p) == "identifier" && ctx.text(p) == name)
                }
                "formal_parameters" => named_children(params).any(|p| {
                    kind(p) == "formal_parameter"
                        && !syntax::has_modifier(p, "final")
                        && named(p.field(Field::Name))
                }),
                _ => false,
            };
            found.then_some("parameter")
        }
        "catch_clause" => {
            let param = named_children(owner).find(|c| kind(*c) == "catch_formal_parameter")?;
            (!syntax::has_modifier(param, "final") && named(param.field(Field::Name)))
                .then_some("caught exception")
        }
        // enhanced for: `for (T x : xs)`; the modifiers sit on the statement.
        "enhanced_for_statement" => (!syntax::has_modifier(owner, "final")
            && named(owner.field(Field::Name)))
        .then_some("loop element"),
        _ => None,
    }
}

fn mentions(node: Node<'_>, name: &str, ctx: &Ctx<'_>) -> bool {
    (kind(node) == "identifier" && ctx.text(node) == name)
        || any_descendant(node, |n| kind(n) == "identifier" && ctx.text(n) == name)
}
