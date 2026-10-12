//! `java/missing-switch-default` — `switch` statement without a `default` label.

use super::util::{self, Fld, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["switch", "defensive"],
    remediation_minutes: 5,
    description: r#"Detects `switch` statements (classic `case X:` or arrow `case X ->` labels) that
have no `default` label, and `switch` statements over an enum declared in the same file that
neither cover all of its constants nor have a `default`.

**Why it matters.** Without `default`, a value that no `case` matches is silently ignored.
When a new value appears later (a new status code, a new string key, a new enum constant)
nothing points at the `switch` that should handle it.

**How to fix.** Add a `default` that handles the remaining values — or throws, if they are
impossible — or document that ignoring them is intended with an empty `default: break;` and
a comment. For an enum, adding the missing constants also works.

```java
// Noncompliant
switch (code) {
    case 200: ok(); break;
    case 404: notFound(); break;
}

// Compliant
switch (code) {
    case 200: ok(); break;
    case 404: notFound(); break;
    default: throw new IllegalStateException("unexpected status " + code);
}
```

Not reported: `switch` expressions (the compiler already requires them to be exhaustive),
pattern switches, switches whose labels are bare names (the names may be enum constants
declared elsewhere, whose complete list is unknown) unless the selector is an enum declared in
this file, or a variable of this file with a primitive or `String` type whose labels are all
constants declared in this file, and enum switches that list every constant."#,
    ..RuleMeta::new(
        "java/missing-switch-default",
        "java",
        "switch statement without default",
    )
};

pub struct MissingSwitchDefault;

/// Whether a `switch_expression` node sits in statement position.
fn is_statement<'t>(ctx: &Ctx<'t>, node: Node<'t>) -> bool {
    ctx.parent(node).is_some_and(|p| {
        matches!(
            kind(p),
            "block"
                | "constructor_body"
                | "switch_block_statement_group"
                | "labeled_statement"
                | "if_statement"
                | "while_statement"
                | "for_statement"
                | "enhanced_for_statement"
                | "do_statement"
        )
    })
}

/// The declaration of the enum `fqn` if it is a top-level or member type of this file.
fn local_enum<'t>(ctx: &Ctx<'t>, fqn: &str) -> Option<Node<'t>> {
    let mut stack: Vec<Node<'t>> = named_children(ctx.root())
        .filter(|c| syntax::is_type_declaration(*c))
        .collect();
    let mut guard = 0;
    while let Some(decl) = stack.pop() {
        guard += 1;
        if guard > 1_000 {
            return None;
        }
        let Some(name) = ctx.class_fqn(decl) else {
            continue;
        };
        if name == fqn {
            return (kind(decl) == "enum_declaration").then_some(decl);
        }
        let nested = fqn
            .strip_prefix(name.as_str())
            .is_some_and(|rest| rest.starts_with('.'));
        if nested {
            if let Some(body) = syntax::type_body(decl) {
                stack.extend(
                    ctx.members(body)
                        .filter(|m| syntax::is_type_declaration(*m)),
                );
            }
        }
    }
    None
}

/// Whether the switch is over a variable declared in this file with a primitive or `String`
/// type, and every label is a `static final` field of this file: a switch over such values is
/// never exhaustive, so the bare names cannot be the constants of an enum.
fn constant_labels_over_value<'t>(ctx: &Ctx<'t>, selector: Node<'t>, labels: &[Node<'t>]) -> bool {
    let mut sel = syntax::strip_parens(selector);
    if kind(sel) == "field_access" {
        match (sel.field(Field::Object), sel.field(Field::Field)) {
            (Some(o), Some(f)) if kind(o) == "this" => sel = f,
            _ => return false,
        }
    }
    if kind(sel) != "identifier" || ctx.declaration_of(sel).is_none() {
        return false;
    }
    let Some(t) = ctx.type_of(sel) else {
        return false;
    };
    if !(t.is_string() || matches!(t, JType::Prim(_))) {
        return false;
    }
    labels.iter().all(|l| {
        ctx.declaration_of(*l).is_some_and(|d| {
            d.kind == VarKind::Field
                && (kind(d.statement) == "constant_declaration"
                    || (syntax::has_modifier(d.statement, "static")
                        && syntax::has_modifier(d.statement, "final")))
        })
    })
}

/// The constants of the enum `selector` has, when it is an enum declared in this file.
fn enum_constants<'t>(ctx: &Ctx<'t>, selector: Node<'t>) -> Option<Vec<&'t str>> {
    let t = ctx.type_of(selector)?;
    let decl = local_enum(ctx, t.fqn()?)?;
    let body = syntax::type_body(decl)?;
    Some(
        ctx.members(body)
            .filter(|m| kind(*m) == "enum_constant")
            .filter_map(|m| m.field(Field::Name))
            .map(|n| ctx.text(n))
            .collect(),
    )
}

impl<'t> JavaRule<'t> for MissingSwitchDefault {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["switch_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(body) = node.fld(F::Body) else {
            return;
        };
        if body.has_error() {
            return;
        }
        let mut labels = 0usize;
        let mut names: Vec<&'t str> = Vec::new();
        let mut name_nodes: Vec<Node<'t>> = Vec::new();
        let mut others = 0usize;
        for group in named_children(body) {
            if !matches!(kind(group), "switch_block_statement_group" | "switch_rule") {
                return;
            }
            for label in named_children(group).filter(|c| kind(*c) == "switch_label") {
                labels += 1;
                if syntax::children(label).any(|c| !c.is_named() && kind(c) == "default") {
                    return;
                }
                for v in named_children(label) {
                    match kind(v) {
                        "identifier" => {
                            names.push(ctx.text(v));
                            name_nodes.push(v);
                        }
                        // Pattern switches and `case null` must be exhaustive anyway.
                        "pattern" | "type_pattern" | "record_pattern" | "guard"
                        | "null_literal" => return,
                        _ => others += 1,
                    }
                }
            }
        }
        if labels == 0 || !is_statement(ctx, node) {
            return;
        }
        let at = util::keyword(node, "switch").unwrap_or(node);
        if names.is_empty() {
            ctx.emit(at, "Add a `default` label to handle values no case matches");
            return;
        }
        // Bare names: enum constants, or constants we can't tell apart from them. Only an enum
        // declared in this file has a known list of constants.
        if others > 0 {
            return;
        }
        let Some(selector) = node.fld(F::Condition) else {
            return;
        };
        let Some(constants) = enum_constants(ctx, selector) else {
            if constant_labels_over_value(ctx, selector, &name_nodes) {
                ctx.emit(at, "Add a `default` label to handle values no case matches");
            }
            return;
        };
        let missing: Vec<&str> = constants
            .iter()
            .filter(|c| !names.contains(c))
            .copied()
            .collect();
        if missing.is_empty() {
            return;
        }
        let mut list = missing
            .iter()
            .take(3)
            .map(|m| format!("`{m}`"))
            .collect::<Vec<_>>()
            .join(", ");
        if missing.len() > 3 {
            list.push_str(&format!(" and {} more", missing.len() - 3));
        }
        ctx.emit(
            at,
            format!("Add a `default` label or cases for the unhandled constants {list}"),
        )
        .value(missing.len() as f64);
    }
}
