//! `js/no-throw-literal`

use crate::prelude::*;

pub struct NoThrowLiteral;

pub static NO_THROW_LITERAL: RuleMeta = RuleMeta {
    severity: Severity::Medium,
    tags: &["error-handling"],
    remediation_minutes: 5,
    description: r#"Flags `throw` statements whose value is certainly not an `Error`: a string, number,
boolean, `null`, `undefined`, template literal, object or array literal, arithmetic or string
concatenation, or a function.

**Why:** only `Error` objects carry a stack trace, and code that handles exceptions usually
expects `error.message` and `error.stack`. A thrown string or plain object makes failures much
harder to diagnose and breaks such handlers.

**Fix:** throw an `Error` (or a subclass): `throw new Error('…')`. Values whose type cannot be
seen from the source (variables, calls, `new X()`) are not reported.

```js
throw 'Invalid state';                   // flagged
throw { code: 42 };                      // flagged
throw `Missing ${name}`;                 // flagged

throw new Error('Invalid state');        // compliant
throw error;                             // compliant: may be an Error
```
"#,
    ..RuleMeta::new(
        "js/no-throw-literal",
        "js",
        "Only 'Error' objects should be thrown",
    )
};

/// Is the thrown value certainly not an `Error` object?
fn is_not_error(ctx: &Ctx<'_>, expr: &Expression<'_>) -> bool {
    match helpers::unwrap_expr(expr) {
        Expression::StringLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BigIntLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::RegExpLiteral(_)
        | Expression::TemplateLiteral(_)
        | Expression::ObjectExpression(_)
        | Expression::ArrayExpression(_)
        | Expression::UnaryExpression(_)
        | Expression::BinaryExpression(_)
        | Expression::UpdateExpression(_)
        | Expression::ArrowFunctionExpression(_)
        | Expression::FunctionExpression(_) => true,
        e @ Expression::Identifier(_) => ctx.is_global(e, "undefined"),
        Expression::ConditionalExpression(c) => {
            is_not_error(ctx, &c.consequent) && is_not_error(ctx, &c.alternate)
        }
        Expression::LogicalExpression(l) => {
            is_not_error(ctx, &l.left) && is_not_error(ctx, &l.right)
        }
        Expression::SequenceExpression(s) => {
            s.expressions.last().is_some_and(|e| is_not_error(ctx, e))
        }
        Expression::AssignmentExpression(a) => {
            a.operator == AssignmentOperator::Assign && is_not_error(ctx, &a.right)
        }
        _ => false,
    }
}

impl JsRule for NoThrowLiteral {
    fn meta(&self) -> &'static RuleMeta {
        &NO_THROW_LITERAL
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::ThrowStatement]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::ThrowStatement(throw) = node.kind() else {
            return;
        };
        if !is_not_error(ctx, &throw.argument) {
            return;
        }
        ctx.emit(
            throw.argument.span(),
            "Throw an 'Error' object instead of this value",
        );
    }
}
