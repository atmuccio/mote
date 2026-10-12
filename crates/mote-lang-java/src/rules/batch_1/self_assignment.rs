//! `java/self-assignment` — a variable or field assigned to itself.

use super::util::{self, Fld, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["suspicious", "typo"],
    remediation_minutes: 5,
    description: r#"Detects assignments whose target and value are the same expression: `x = x;`,
`this.x = this.x;`, `a[i] = a[i];`.

**Why it matters.** Such a statement does nothing. It almost always hides a typo: the classic
case is a constructor or setter whose parameter has the same name as the field, where
`value = value;` assigns the parameter to itself and leaves the field untouched.

**How to fix.** Assign what was meant (usually `this.value = value;`) or delete the statement.

```java
// Noncompliant
Point(int x) {
    x = x;              // the field keeps its default value
}

// Compliant
Point(int x) {
    this.x = x;
}
```

`this.x = x` is not reported when `x` is a parameter or local (it assigns that value to the
field); it is reported when no such variable is in scope, so `x` is the field itself — usually a
misspelled parameter (`void setLimit(int limt) { this.limit = limit; }`). Targets with side
effects (`a[i++] = a[i++]`, calls) are not reported."#,
    ..RuleMeta::new(
        "java/self-assignment",
        "java",
        "Variable assigned to itself",
    )
};

pub struct SelfAssignment;

/// `this.name = name` (or `name = this.name`) where the bare `name` resolves to the field of the
/// innermost class itself, because no parameter or local of that name is in scope — typically
/// a misspelled setter or constructor parameter. Returns the field name.
fn field_to_itself<'t>(
    assignment: Node<'t>,
    left: Node<'t>,
    right: Node<'t>,
    ctx: &Ctx<'t>,
) -> Option<&'t str> {
    let right = syntax::strip_parens(right);
    let (access, bare) = match (kind(left), kind(right)) {
        ("field_access", "identifier") => (left, right),
        ("identifier", "field_access") => (right, left),
        _ => return None,
    };
    let (object, field) = (access.fld(F::Object)?, access.fld(F::Field)?);
    let name = ctx.text(bare);
    if kind(object) != "this" || ctx.text(field) != name {
        return None;
    }
    // `this.x = x` in a method with a parameter `x` (the usual constructor or setter): the
    // bare name resolves to that parameter (or to something more local), never to the field,
    // so the resolver need not be asked.
    if ctx
        .enclosing_method(assignment)
        .is_some_and(|m| has_parameter(m, name, ctx))
    {
        return None;
    }
    let decl = ctx.declaration_of(bare)?;
    if decl.kind != VarKind::Field {
        return None;
    }
    // `this` is the innermost class, so the field must be declared directly in its body (not
    // in an enclosing class of a nested or anonymous one).
    let body = ctx.ancestors(assignment).find(|a| {
        matches!(
            kind(*a),
            "class_body"
                | "interface_body"
                | "enum_body"
                | "enum_body_declarations"
                | "annotation_type_body"
        )
    })?;
    (decl.statement.parent() == Some(body)).then_some(name)
}

/// Whether `method` declares a parameter named `name` (as the resolver's scopes list them:
/// formal and variable-arity parameters).
fn has_parameter<'t>(method: Node<'t>, name: &str, ctx: &Ctx<'t>) -> bool {
    let Some(params) = method.field(Field::Parameters) else {
        return false;
    };
    named_children(params).any(|p| {
        let id = match kind(p) {
            "formal_parameter" => p.field(Field::Name),
            "spread_parameter" => named_children(p)
                .find(|c| kind(*c) == "variable_declarator")
                .and_then(|d| d.field(Field::Name)),
            _ => None,
        };
        id.is_some_and(|id| ctx.text(id) == name)
    })
}

impl<'t> JavaRule<'t> for SelfAssignment {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["assignment_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if util::op_str(node) != "=" {
            return;
        }
        let (Some(left), Some(right)) = (node.fld(F::Left), node.fld(F::Right)) else {
            return;
        };
        if !matches!(kind(left), "identifier" | "field_access" | "array_access") {
            return;
        }
        if !util::same_code(left, right, ctx.source()) {
            if let Some(name) = field_to_itself(node, left, right, ctx) {
                ctx.emit(
                    node,
                    format!("Remove this assignment of field `{name}` to itself, or assign the intended value (is a parameter name misspelled?)"),
                );
            }
            return;
        }
        if util::has_side_effects(left) {
            return;
        }
        let target = ctx.text(left);
        let is_param = kind(left) == "identifier"
            && ctx
                .declaration_of(left)
                .is_some_and(|d| d.kind == VarKind::Parameter);
        let msg = if is_param {
            format!("Remove this assignment of `{target}` to itself, or assign the intended value (e.g. `this.{target} = {target}`)")
        } else {
            format!("Remove this assignment of `{target}` to itself, or assign the intended value")
        };
        ctx.emit(node, msg);
    }
}
