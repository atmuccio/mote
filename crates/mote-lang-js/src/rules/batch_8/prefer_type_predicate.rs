//! `js/prefer-type-predicate`

use crate::prelude::*;

pub struct PreferTypePredicate;

pub static PREFER_TYPE_PREDICATE: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Incubating,
    tags: &["typescript", "types"],
    remediation_minutes: 2,
    description: r#"Flags TypeScript functions declared to return `boolean` whose whole body is a `typeof` or
`instanceof` test of their parameter: `function isDate(v: unknown): boolean { return v instanceof
Date; }`.

**Why:** such a function is a type guard in all but name. Declared as returning `boolean`, it tells
the compiler nothing, so callers that test it still have to cast (`if (isDate(v)) (v as
Date).getTime()`). Declared as a type predicate (`v is Date`), it narrows the argument wherever it
is called. Without the `: boolean` annotation, TypeScript infers the predicate by itself; the
annotation is what turns it off.

**Fix:** declare the return type as a predicate on the parameter, or remove the annotation.

```ts
function isDate(v: unknown): boolean {              // flagged
  return v instanceof Date;
}

function isDate(v: unknown): v is Date {            // compliant
  return v instanceof Date;
}
```

Only a single `return` (or an arrow's expression body) testing a parameter is reported, with
`typeof` against `'string'`, `'number'`, `'boolean'`, `'bigint'`, `'symbol'`, `'undefined'` or
`'function'`, or with `instanceof`. Not reported: negated tests (`!==`, `!(x instanceof Y)`), which
narrow in the other branch, `typeof … === 'object'` (true for `null` too), combinations with `&&`
or `||`, destructured parameters, signatures without a body, and methods other than private ones
(a public or overriding method usually implements a boolean contract, such as `matches(other)`,
that callers reach through the base type, where a predicate narrows nothing).
"#,
    ..RuleMeta::new(
        "js/prefer-type-predicate",
        "js",
        "Type-testing functions should be declared as type predicates",
    )
};

impl JsRule for PreferTypePredicate {
    fn meta(&self) -> &'static RuleMeta {
        &PREFER_TYPE_PREDICATE
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.is_typescript()
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::Function, AstType::ArrowFunctionExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let (params, return_type, statements) = match node.kind() {
            AstKind::Function(f) => {
                let Some(body) = &f.body else {
                    return;
                };
                (
                    &f.params,
                    f.return_type.as_deref(),
                    body.statements.as_slice(),
                )
            }
            AstKind::ArrowFunctionExpression(a) => (
                &a.params,
                a.return_type.as_deref(),
                a.body.statements.as_slice(),
            ),
            _ => return,
        };
        let Some(return_type) = return_type else {
            return;
        };
        if !matches!(return_type.type_annotation, TSType::TSBooleanKeyword(_)) {
            return;
        }
        let returned = match statements {
            [Statement::ReturnStatement(r)] => match &r.argument {
                Some(e) => e,
                None => return,
            },
            [Statement::ExpressionStatement(e)] if is_expression_body(node) => &e.expression,
            _ => return,
        };
        let Some((tested, narrowed)) = tested_parameter(ctx, returned) else {
            return;
        };
        let is_param = params.items.iter().any(|p| {
            matches!(&p.pattern, BindingPattern::BindingIdentifier(b) if b.name == tested)
                && p.initializer.is_none()
        });
        if !is_param || !own_signature(ctx, node) {
            return;
        }
        ctx.emit(
            return_type.span,
            format!("Declare the return type as '{tested} is {narrowed}' so that callers get the narrowed type"),
        );
    }
}

/// Is the signature the function's own to choose? Public class methods and object methods often
/// implement or override a boolean-returning contract (`matches`, `canSerialize`) that callers
/// use through the base type, where a predicate narrows nothing; only private methods are checked.
fn own_signature(ctx: &Ctx<'_>, node: &AstNode<'_>) -> bool {
    match ctx.parent_kind(node) {
        Some(AstKind::MethodDefinition(m)) => {
            !m.r#override
                && (m.accessibility == Some(TSAccessibility::Private)
                    || matches!(m.key, PropertyKey::PrivateIdentifier(_)))
        }
        Some(AstKind::ObjectProperty(_)) => false,
        Some(AstKind::PropertyDefinition(p)) => {
            !p.r#override
                && (p.accessibility == Some(TSAccessibility::Private)
                    || matches!(p.key, PropertyKey::PrivateIdentifier(_)))
        }
        _ => true,
    }
}

fn is_expression_body(node: &AstNode<'_>) -> bool {
    matches!(node.kind(), AstKind::ArrowFunctionExpression(a) if a.expression)
}

/// `typeof p === 'string'` → `("p", "string")`; `p instanceof Date` → `("p", "Date")`.
fn tested_parameter<'a>(ctx: &Ctx<'a>, expr: &Expression<'a>) -> Option<(&'a str, &'a str)> {
    let Expression::BinaryExpression(b) = helpers::unwrap_expr(expr) else {
        return None;
    };
    match b.operator {
        BinaryOperator::Instanceof => {
            let Expression::Identifier(p) = helpers::unwrap_expr(&b.left) else {
                return None;
            };
            let class = match helpers::unwrap_expr(&b.right) {
                Expression::Identifier(c) => c.name.as_str(),
                Expression::StaticMemberExpression(_) => ctx.text(b.right.span()),
                _ => return None,
            };
            Some((p.name.as_str(), class))
        }
        BinaryOperator::StrictEquality | BinaryOperator::Equality => {
            let (typeof_side, literal_side) = if helpers::is_typeof(&b.left) {
                (&b.left, &b.right)
            } else if helpers::is_typeof(&b.right) {
                (&b.right, &b.left)
            } else {
                return None;
            };
            let Expression::UnaryExpression(u) = helpers::unwrap_expr(typeof_side) else {
                return None;
            };
            let Expression::Identifier(p) = helpers::unwrap_expr(&u.argument) else {
                return None;
            };
            let kind = helpers::string_value(literal_side)?;
            let narrowed = match kind {
                "string" | "number" | "boolean" | "bigint" | "symbol" | "undefined" => kind,
                "function" => "Function",
                _ => return None,
            };
            Some((p.name.as_str(), narrowed))
        }
        _ => None,
    }
}
