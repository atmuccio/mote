//! `js/accessor-field-mismatch`

use crate::prelude::*;
use crate::rules::batch_6::util::descendants;

pub struct AccessorFieldMismatch;

pub static ACCESSOR_FIELD_MISMATCH: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["suspicious"],
    remediation_minutes: 5,
    description: r#"Flags a class getter or setter that reads or writes the field of *another* accessor of the class,
while the class has a field that matches its own name: `get width() { return this._height; }` in
a class with a `_width` field and a `height` accessor.

**Why:** accessors backed by a field are written by copy and paste, and forgetting to rename the
field in the copy is easy. The mistake is silent: the property returns another property's value,
or a setter stores into the wrong field, and the matching field is left stale.

**Fix:** use the field that matches the accessor's name.

```js
class Box {
  _width = 0;
  _height = 0;
  get width() { return this._height; }       // flagged
  set width(value) { this._height = value; } // flagged
  get height() { return this._height; }      // compliant
}
```

Only accessors whose whole body is `return this.x;` (getter) or `this.x = value;` (setter) are
checked, and only when the class declares or assigns a field named like the accessor with a `_`
or `#` prefix or a `_` suffix. An accessor that exposes a differently named field which belongs to
no other accessor (`get tools() { return this._visibleTools; }` next to a raw `_tools`) is a
deliberate design and is not reported; nor are static accessors.
"#,
    ..RuleMeta::new(
        "js/accessor-field-mismatch",
        "js",
        "Getters and setters should use the field that matches their name",
    )
};

impl JsRule for AccessorFieldMismatch {
    fn meta(&self) -> &'static RuleMeta {
        &ACCESSOR_FIELD_MISMATCH
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::Class]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::Class(class) = node.kind() else {
            return;
        };
        // Cheap check first: any instance accessor with a one-statement body?
        let has_candidate = class.body.body.iter().any(|element| {
            matches!(element, ClassElement::MethodDefinition(m)
                if matches!(m.kind, MethodDefinitionKind::Get | MethodDefinitionKind::Set)
                    && !m.r#static
                    && m.value.body.as_ref().is_some_and(|b| b.statements.len() == 1))
        });
        if !has_candidate {
            return;
        }
        // Names of the instance accessors: a copied accessor uses the field of one of them.
        let accessors: Vec<Cow<'_, str>> = class
            .body
            .body
            .iter()
            .filter_map(|element| match element {
                ClassElement::MethodDefinition(m)
                    if matches!(
                        m.kind,
                        MethodDefinitionKind::Get | MethodDefinitionKind::Set
                    ) && !m.r#static
                        && !m.computed =>
                {
                    m.key.static_name()
                }
                _ => None,
            })
            .collect();
        // The class's fields are only needed once an accessor uses another accessor's field.
        let mut fields: Option<Vec<String>> = None;
        let mut found: Vec<(Span, String)> = Vec::new();
        for element in &class.body.body {
            let ClassElement::MethodDefinition(method) = element else {
                continue;
            };
            if method.r#static || method.computed {
                continue;
            }
            let Some(name) = method.key.static_name() else {
                continue;
            };
            let Some(body) = &method.value.body else {
                continue;
            };
            let [statement] = body.statements.as_slice() else {
                continue;
            };
            let used = match method.kind {
                MethodDefinitionKind::Get => getter_field(statement),
                MethodDefinitionKind::Set => setter_field(statement, &method.value),
                _ => None,
            };
            let Some((used, used_span)) = used else {
                continue;
            };
            if used == name.as_ref() || strip(&used) == name.as_ref() {
                continue;
            }
            // `get tools() { return this._gatedTools; }` exposes a derived field on purpose; a
            // copy-and-paste slip uses the field of another accessor (`get width()` returning
            // `this._height` next to `get height()`).
            let source = strip(&used);
            if !accessors.iter().any(|a| a.as_ref() == source) {
                continue;
            }
            let Some(matching) = fields
                .get_or_insert_with(|| instance_fields(ctx, node, class))
                .iter()
                .find(|f| f.as_str() != name.as_ref() && strip(f) == name.as_ref())
            else {
                continue;
            };
            let what = if method.kind == MethodDefinitionKind::Get {
                "getter"
            } else {
                "setter"
            };
            let verb = if method.kind == MethodDefinitionKind::Get {
                "reads"
            } else {
                "writes"
            };
            found.push((
                used_span,
                format!(
                    "The {what} '{name}' {verb} '{used}', the field of the accessor '{source}', although the class has a field '{matching}' that matches its name: use '{matching}' if this was copied from '{source}'"
                ),
            ));
        }
        for (span, message) in found {
            ctx.emit(span, message);
        }
    }
}

/// `return this.x;` → `x` (`#x` for a private field).
fn getter_field<'a>(statement: &Statement<'a>) -> Option<(String, Span)> {
    let Statement::ReturnStatement(ret) = statement else {
        return None;
    };
    this_member(ret.argument.as_ref()?)
}

/// `this.x = value;` where `value` is the setter's parameter → `x`.
fn setter_field<'a>(statement: &Statement<'a>, function: &Function<'a>) -> Option<(String, Span)> {
    let Statement::ExpressionStatement(e) = statement else {
        return None;
    };
    let Expression::AssignmentExpression(assign) = helpers::unwrap_expr(&e.expression) else {
        return None;
    };
    if assign.operator != AssignmentOperator::Assign {
        return None;
    }
    let param = match function.params.items.first().map(|p| &p.pattern) {
        Some(BindingPattern::BindingIdentifier(id)) => id.name.as_str(),
        _ => return None,
    };
    if !matches!(helpers::unwrap_expr(&assign.right), Expression::Identifier(id) if id.name == param)
    {
        return None;
    }
    match &assign.left {
        AssignmentTarget::StaticMemberExpression(m)
            if matches!(m.object, Expression::ThisExpression(_)) =>
        {
            Some((m.property.name.to_string(), m.property.span))
        }
        AssignmentTarget::PrivateFieldExpression(m)
            if matches!(m.object, Expression::ThisExpression(_)) =>
        {
            Some((format!("#{}", m.field.name), m.field.span))
        }
        _ => None,
    }
}

/// `this.x` or `this.#x`.
fn this_member(expr: &Expression<'_>) -> Option<(String, Span)> {
    match helpers::unwrap_expr(expr) {
        Expression::StaticMemberExpression(m)
            if matches!(m.object, Expression::ThisExpression(_)) =>
        {
            Some((m.property.name.to_string(), m.property.span))
        }
        Expression::PrivateFieldExpression(m)
            if matches!(m.object, Expression::ThisExpression(_)) =>
        {
            Some((format!("#{}", m.field.name), m.field.span))
        }
        _ => None,
    }
}

/// The name without a leading `_`/`#`/`$` or a trailing `_`: `_width`, `#width`, `width_` → `width`.
fn strip(name: &str) -> &str {
    name.trim_start_matches(['_', '#', '$'])
        .trim_end_matches('_')
}

/// Instance fields of the class: declared properties, TypeScript parameter properties, and every
/// `this.x` / `this.#x` used in instance members (constructor included).
fn instance_fields(ctx: &Ctx<'_>, node: &AstNode<'_>, class: &Class<'_>) -> Vec<String> {
    let mut fields: Vec<String> = Vec::new();
    for element in &class.body.body {
        match element {
            ClassElement::PropertyDefinition(p) if !p.r#static => {
                if let Some(name) = p.key.static_name() {
                    fields.push(name.to_string());
                } else if let PropertyKey::PrivateIdentifier(id) = &p.key {
                    fields.push(format!("#{}", id.name));
                }
            }
            ClassElement::AccessorProperty(p) if !p.r#static => {
                if let Some(name) = p.key.static_name() {
                    fields.push(name.to_string());
                }
            }
            ClassElement::MethodDefinition(m) if m.kind == MethodDefinitionKind::Constructor => {
                for param in &m.value.params.items {
                    if param.accessibility.is_none() && !param.readonly {
                        continue;
                    }
                    if let BindingPattern::BindingIdentifier(id) = &param.pattern {
                        fields.push(id.name.to_string());
                    }
                }
            }
            _ => {}
        }
    }
    // `this.x` anywhere in the class body, except inside nested classes and `function`s (their
    // `this` is another object); arrow functions keep the class's `this`.
    let mut skip_until = 0;
    let mut static_until = 0;
    for inner in descendants(ctx, node) {
        let span = inner.span();
        if span.start < skip_until {
            continue;
        }
        match inner.kind() {
            AstKind::Class(_) => skip_until = span.end,
            AstKind::MethodDefinition(m) if m.r#static => static_until = span.end,
            AstKind::PropertyDefinition(p) if p.r#static => static_until = span.end,
            AstKind::StaticBlock(_) => static_until = span.end,
            AstKind::Function(_) => {
                // Methods are `Function` nodes under a `MethodDefinition`; other functions rebind
                // `this`.
                if !matches!(ctx.parent_kind(inner), Some(AstKind::MethodDefinition(_))) {
                    skip_until = span.end;
                }
            }
            AstKind::StaticMemberExpression(m)
                if span.start >= static_until
                    && matches!(m.object, Expression::ThisExpression(_)) =>
            {
                fields.push(m.property.name.to_string());
            }
            AstKind::PrivateFieldExpression(m)
                if span.start >= static_until
                    && matches!(m.object, Expression::ThisExpression(_)) =>
            {
                fields.push(format!("#{}", m.field.name));
            }
            _ => {}
        }
    }
    fields.sort();
    fields.dedup();
    fields
}
