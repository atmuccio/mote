//! `js/no-self-assign`

use super::util;
use crate::prelude::*;
use oxc_span::ContentEq;

pub struct NoSelfAssign;

pub static NO_SELF_ASSIGN: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["suspicious"],
    remediation_minutes: 5,
    description: r#"Flags assignments whose target and value are the same: `x = x`, `[a, b] = [a, b]`,
`({ a } = { a })`, and property self-assignments such as `this.name = this.name` when they are
evidently a typo.

**Why:** assigning a variable to itself does nothing. It is nearly always an incomplete edit — the
right-hand side was meant to be a parameter, another variable, or a freshly computed value — so the
intended update silently never happens. A destructuring "swap" written with both sides in the same
order is the same mistake.

**Fix:** assign the value that was meant (often a parameter or a differently named variable), or
delete the statement.

```js
function setName(name) {
  this.name = this.name;   // flagged: should be `this.name = name`
}

[first, second] = [first, second]; // flagged: not a swap
[first, second] = [second, first]; // compliant
```

Assigning a property to itself can deliberately re-run a setter (`picker.items = picker.items`,
`canvas.width = canvas.width`, `location.href = location.href`), so a standalone property
self-assignment is only reported when there is evidence of a typo: a variable or parameter with
the property's name is in scope, or the object is `this` in a class that has no base class and no
setter or decorated field with that name.
"#,
    ..RuleMeta::new(
        "js/no-self-assign",
        "js",
        "Variables and properties should not be assigned to themselves",
    )
};

/// Properties whose setter has a visible effect even when the value does not change (reload,
/// re-fetch, clearing a canvas, re-layout, …).
const SETTER_IDIOMS: &[&str] = &[
    "href",
    "src",
    "srcset",
    "value",
    "scrollTop",
    "scrollLeft",
    "innerHTML",
    "outerHTML",
    "textContent",
    "innerText",
    "hash",
    "search",
    "pathname",
    "width",
    "height",
    "location",
    "currentTime",
    "data",
    "className",
    "checked",
    "selectedIndex",
    "files",
];

/// The target of a standalone `obj.prop = obj.prop`.
#[derive(Clone, Copy)]
struct Member<'a> {
    is_this: bool,
    property: Option<&'a str>,
    private: bool,
}

/// A self-assigned value found in an assignment.
struct Found<'a> {
    span: Span,
    /// Set for a standalone property self-assignment, which needs evidence of a typo.
    member: Option<Member<'a>>,
}

impl JsRule for NoSelfAssign {
    fn meta(&self) -> &'static RuleMeta {
        &NO_SELF_ASSIGN
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::AssignmentExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::AssignmentExpression(assign) = node.kind() else {
            return;
        };
        if !matches!(
            assign.operator,
            AssignmentOperator::Assign
                | AssignmentOperator::LogicalAnd
                | AssignmentOperator::LogicalOr
                | AssignmentOperator::LogicalNullish
        ) {
            return;
        }
        let mut found: Vec<Found<'a>> = Vec::new();
        check_target(&assign.left, &assign.right, true, &mut found);
        for f in found {
            if let Some(member) = f.member {
                if !member_is_typo(ctx, node, member) {
                    continue;
                }
            }
            let name = util::excerpt(ctx.text(f.span));
            ctx.emit(
                f.span,
                format!(
                    "Remove this assignment of '{name}' to itself or assign the intended value"
                ),
            );
        }
    }
}

/// Collect the right-hand values that are assigned to themselves. `standalone` is false inside a
/// destructuring pattern.
fn check_target<'a>(
    target: &AssignmentTarget<'a>,
    value: &Expression<'a>,
    standalone: bool,
    found: &mut Vec<Found<'a>>,
) {
    let member = |is_this: bool, property: Option<&'a str>, private: bool| {
        standalone.then_some(Member {
            is_this,
            property,
            private,
        })
    };
    match target {
        AssignmentTarget::AssignmentTargetIdentifier(id) => {
            if let Expression::Identifier(v) = value {
                if v.name == id.name {
                    found.push(Found {
                        span: v.span,
                        member: None,
                    });
                }
            }
        }
        AssignmentTarget::StaticMemberExpression(m) => {
            if let Expression::StaticMemberExpression(v) = value {
                let property = m.property.name.as_str();
                if !SETTER_IDIOMS.contains(&property) && util::is_pure(&m.object) && m.content_eq(v)
                {
                    let is_this = matches!(m.object, Expression::ThisExpression(_));
                    found.push(Found {
                        span: v.span,
                        member: member(is_this, Some(property), false),
                    });
                }
            }
        }
        AssignmentTarget::ComputedMemberExpression(m) => {
            if let Expression::ComputedMemberExpression(v) = value {
                let property = helpers::string_value(&m.expression);
                let idiom = property.is_some_and(|name| SETTER_IDIOMS.contains(&name));
                if !idiom
                    && util::is_pure(&m.object)
                    && util::is_pure(&m.expression)
                    && m.content_eq(v)
                {
                    let is_this = matches!(m.object, Expression::ThisExpression(_));
                    found.push(Found {
                        span: v.span,
                        member: member(is_this, property, false),
                    });
                }
            }
        }
        AssignmentTarget::PrivateFieldExpression(m) => {
            if let Expression::PrivateFieldExpression(v) = value {
                if util::is_pure(&m.object) && m.content_eq(v) {
                    let is_this = matches!(m.object, Expression::ThisExpression(_));
                    let property = Some(m.field.name.as_str());
                    found.push(Found {
                        span: v.span,
                        member: member(is_this, property, true),
                    });
                }
            }
        }
        AssignmentTarget::ArrayAssignmentTarget(pattern) => {
            let Expression::ArrayExpression(array) = value else {
                return;
            };
            for (element, item) in pattern.elements.iter().zip(array.elements.iter()) {
                // A spread on the right shifts every later position.
                if matches!(item, ArrayExpressionElement::SpreadElement(_)) {
                    break;
                }
                let (Some(element), Some(item)) = (element, item.as_expression()) else {
                    continue;
                };
                if let Some(element) = element.as_assignment_target() {
                    check_target(element, item, false, found);
                }
            }
        }
        AssignmentTarget::ObjectAssignmentTarget(pattern) => {
            let Expression::ObjectExpression(object) = value else {
                return;
            };
            // Spreads and computed keys make the source of each property uncertain.
            let simple = object.properties.iter().all(|p| match p {
                ObjectPropertyKind::ObjectProperty(p) => {
                    p.kind == PropertyKind::Init
                        && !p.method
                        && helpers::property_key_name(&p.key).is_some()
                }
                ObjectPropertyKind::SpreadProperty(_) => false,
            });
            if !simple {
                return;
            }
            for property in &pattern.properties {
                match property {
                    AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(p) => {
                        if p.init.is_some() {
                            continue;
                        }
                        if let Some(Expression::Identifier(v)) =
                            last_value(object, p.binding.name.as_str())
                        {
                            if v.name == p.binding.name {
                                found.push(Found {
                                    span: v.span,
                                    member: None,
                                });
                            }
                        }
                    }
                    AssignmentTargetProperty::AssignmentTargetPropertyProperty(p) => {
                        let Some(key) = helpers::property_key_name(&p.name) else {
                            continue;
                        };
                        let Some(target) = p.binding.as_assignment_target() else {
                            continue;
                        };
                        if let Some(v) = last_value(object, &key) {
                            check_target(target, v, false, found);
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

/// Value of the last property named `key` in an object literal (later keys win).
fn last_value<'b, 'a>(object: &'b ObjectExpression<'a>, key: &str) -> Option<&'b Expression<'a>> {
    object.properties.iter().rev().find_map(|p| match p {
        ObjectPropertyKind::ObjectProperty(p)
            if helpers::property_key_name(&p.key).is_some_and(|k| k == key) =>
        {
            Some(&p.value)
        }
        _ => None,
    })
}

/// Is a standalone `obj.prop = obj.prop` evidently a typo rather than a setter re-trigger?
fn member_is_typo(ctx: &Ctx<'_>, node: &AstNode<'_>, member: Member<'_>) -> bool {
    let Member {
        is_this,
        property,
        private,
    } = member;
    // `this.name = this.name` next to a `name` parameter or variable: `= name` was meant.
    if let Some(name) = property.filter(|_| !private) {
        if let Some(symbol) = ctx.scoping().find_binding(node.scope_id(), name.into()) {
            if ctx.symbol_flags(symbol).intersects(SymbolFlags::Variable) {
                return true;
            }
        }
    }
    if !is_this {
        return false;
    }
    let Some(class) = enclosing_class(ctx, node) else {
        return false;
    };
    if class.super_class.is_some() {
        return false;
    }
    let Some(property) = property else {
        return false;
    };
    // A setter, an auto-accessor, or a decorated (possibly reactive) field of that name makes the
    // assignment meaningful.
    !class.body.body.iter().any(|element| match element {
        ClassElement::MethodDefinition(m) => {
            m.kind == MethodDefinitionKind::Set && key_is(&m.key, property, private)
        }
        ClassElement::AccessorProperty(p) => key_is(&p.key, property, private),
        ClassElement::PropertyDefinition(p) => {
            !p.decorators.is_empty() && key_is(&p.key, property, private)
        }
        _ => false,
    })
}

/// The class whose instance `this` refers to at `node`: the class of the nearest enclosing
/// non-arrow function when that function is a method, or of a field initializer.
fn enclosing_class<'a>(ctx: &Ctx<'a>, node: &AstNode<'_>) -> Option<&'a Class<'a>> {
    let mut in_method = false;
    for ancestor in ctx.ancestors(node.id()) {
        match ancestor.kind() {
            AstKind::Function(_) if !in_method => {
                if !matches!(
                    ctx.parent_kind(ancestor),
                    Some(AstKind::MethodDefinition(_))
                ) {
                    return None;
                }
                in_method = true;
            }
            AstKind::PropertyDefinition(_)
            | AstKind::AccessorProperty(_)
            | AstKind::StaticBlock(_) => {
                in_method = true;
            }
            AstKind::Class(class) if in_method => return Some(class),
            _ => {}
        }
    }
    None
}

fn key_is(key: &PropertyKey<'_>, name: &str, private: bool) -> bool {
    match key {
        PropertyKey::PrivateIdentifier(id) => private && id.name == name,
        _ => !private && key.is_specific_static_name(name),
    }
}
