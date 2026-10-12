//! `js/no-inferrable-types`

use crate::prelude::*;

pub struct NoInferrableTypes;

pub static NO_INFERRABLE_TYPES: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["typescript", "redundant", "conventions"],
    remediation_minutes: 1,
    description: r#"Flags explicit primitive type annotations on variables, parameters with a default
value, and class properties whose initializer is a literal of exactly that type
(`let count: number = 0`).

**Why:** TypeScript infers `number` from `0`, `string` from `'x'`, `boolean` from `true`, and so on.
Writing the type as well only repeats what the initializer already says, adds noise to every
declaration, and must be kept in sync by hand.

**Fix:** remove the annotation and let the type be inferred.

Covered pairs: `string` with a string or template literal; `number` with a numeric literal (also
`-1`, `Infinity`, `NaN`); `boolean` with `true`, `false`, or `!expr`; `bigint` with a bigint
literal; `symbol` with `Symbol()`; `RegExp` with a regular-expression literal. `readonly` and
optional class properties are not flagged (there the annotation changes the inferred type), nor are
`null`/`undefined` annotations (without strict null checks they would widen to `any`).

```ts
let retries: number = 3;                 // flagged
function greet(name: string = 'world') {} // flagged
class Timer { private running: boolean = false; } // flagged

let retries = 3;                         // compliant
let mode: 'fast' | 'slow' = 'fast';      // compliant: a narrower type than inferred
class Timer { readonly limit: number = 5; } // compliant: would infer the literal type 5
```
"#,
    // Stylistic and noisy on real code: opt-in (`extends: all` or enable explicitly).
    default_enabled: false,
    ..RuleMeta::new(
        "js/no-inferrable-types",
        "js",
        "Types that are inferred from a literal initializer should not be written out",
    )
};

impl JsRule for NoInferrableTypes {
    fn meta(&self) -> &'static RuleMeta {
        &NO_INFERRABLE_TYPES
    }

    fn types(&self) -> &'static [AstType] {
        &[
            AstType::VariableDeclarator,
            AstType::FormalParameter,
            AstType::PropertyDefinition,
        ]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.is_typescript()
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let (annotation, init) = match node.kind() {
            AstKind::VariableDeclarator(decl) => {
                if !matches!(decl.id, BindingPattern::BindingIdentifier(_)) {
                    return;
                }
                match (&decl.type_annotation, &decl.init) {
                    (Some(annotation), Some(init)) => (annotation, init),
                    _ => return,
                }
            }
            AstKind::FormalParameter(param) => {
                if !matches!(param.pattern, BindingPattern::BindingIdentifier(_)) {
                    return;
                }
                match (&param.type_annotation, &param.initializer) {
                    (Some(annotation), Some(init)) => (annotation, &**init),
                    _ => return,
                }
            }
            AstKind::PropertyDefinition(prop) => {
                if prop.readonly || prop.optional || prop.declare {
                    return;
                }
                match (&prop.type_annotation, &prop.value) {
                    (Some(annotation), Some(init)) => (annotation, init),
                    _ => return,
                }
            }
            _ => return,
        };
        let Some(keyword) = inferred_keyword(ctx, &annotation.type_annotation, init) else {
            return;
        };
        ctx.emit(
            annotation.type_annotation.span(),
            format!("Remove the type annotation '{keyword}': it is inferred from the initializer"),
        );
    }
}

/// The annotation's type name when `init` is a literal of exactly that primitive type.
fn inferred_keyword(ctx: &Ctx<'_>, ty: &TSType<'_>, init: &Expression<'_>) -> Option<&'static str> {
    match ty {
        TSType::TSStringKeyword(_) => matches!(
            init,
            Expression::StringLiteral(_) | Expression::TemplateLiteral(_)
        )
        .then_some("string"),
        TSType::TSNumberKeyword(_) => is_number_literal(ctx, init).then_some("number"),
        TSType::TSBooleanKeyword(_) => match init {
            Expression::BooleanLiteral(_) => Some("boolean"),
            Expression::UnaryExpression(u) if u.operator == UnaryOperator::LogicalNot => {
                Some("boolean")
            }
            _ => None,
        },
        TSType::TSBigIntKeyword(_) => match init {
            Expression::BigIntLiteral(_) => Some("bigint"),
            Expression::UnaryExpression(u)
                if u.operator == UnaryOperator::UnaryNegation
                    && matches!(u.argument, Expression::BigIntLiteral(_)) =>
            {
                Some("bigint")
            }
            _ => None,
        },
        TSType::TSSymbolKeyword(_) => match init {
            Expression::CallExpression(call) if ctx.is_global(&call.callee, "Symbol") => {
                Some("symbol")
            }
            _ => None,
        },
        TSType::TSTypeReference(r) if r.type_arguments.is_none() => match &r.type_name {
            TSTypeName::IdentifierReference(id)
                if id.name == "RegExp"
                    && ctx.is_global_reference(id)
                    && matches!(init, Expression::RegExpLiteral(_)) =>
            {
                Some("RegExp")
            }
            _ => None,
        },
        _ => None,
    }
}

/// `1`, `-1`, `+1`, `Infinity`, `-Infinity`, `NaN` (the globals, not shadowed).
fn is_number_literal(ctx: &Ctx<'_>, init: &Expression<'_>) -> bool {
    match init {
        Expression::NumericLiteral(_) => true,
        Expression::Identifier(_) => ctx.is_global(init, "Infinity") || ctx.is_global(init, "NaN"),
        Expression::UnaryExpression(u)
            if matches!(
                u.operator,
                UnaryOperator::UnaryNegation | UnaryOperator::UnaryPlus
            ) =>
        {
            match &u.argument {
                Expression::NumericLiteral(_) => true,
                arg @ Expression::Identifier(_) => {
                    ctx.is_global(arg, "Infinity") || ctx.is_global(arg, "NaN")
                }
                _ => false,
            }
        }
        _ => false,
    }
}
