//! `js/no-extra-boolean-cast`

use super::util;
use crate::prelude::*;

pub struct NoExtraBooleanCast;

pub static NO_EXTRA_BOOLEAN_CAST: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["redundant"],
    remediation_minutes: 1,
    description: r#"Flags `!!x` and `Boolean(x)` where the result is used in a place that already
converts its value to a boolean: the condition of `if`, `while`, `do … while`, `for`, or `?:`,
the operand of `!`, and the argument of `Boolean(…)`.

**Why:** those contexts test truthiness anyway, so the conversion is noise that makes the
condition longer and suggests the author was unsure how it works.

**Fix:** remove the double negation or the `Boolean` call. Keep them where a real boolean value
is needed, such as a return value, an assignment, or a property.

```js
if (!!user) {}              // flagged
while (Boolean(next)) {}    // flagged

if (user) {}                // compliant
const hasUser = !!user;     // compliant: stores a boolean
```
"#,
    ..RuleMeta::new(
        "js/no-extra-boolean-cast",
        "js",
        "Redundant boolean conversions should be removed",
    )
};

fn is_not<'b, 'a>(expr: &'b Expression<'a>) -> Option<&'b Expression<'a>> {
    match helpers::unwrap_expr(expr) {
        Expression::UnaryExpression(u) if u.operator == UnaryOperator::LogicalNot => {
            Some(&u.argument)
        }
        _ => None,
    }
}

/// Is the expression spanning `span` (a child of `parent`) in a position that coerces to boolean?
fn in_boolean_context(ctx: &Ctx<'_>, parent: &AstNode<'_>, span: Span) -> bool {
    match parent.kind() {
        AstKind::IfStatement(s) => s.test.span() == span,
        AstKind::WhileStatement(s) => s.test.span() == span,
        AstKind::DoWhileStatement(s) => s.test.span() == span,
        AstKind::ForStatement(s) => s.test.as_ref().is_some_and(|t| t.span() == span),
        AstKind::ConditionalExpression(c) => c.test.span() == span,
        AstKind::UnaryExpression(u) => u.operator == UnaryOperator::LogicalNot,
        AstKind::CallExpression(c) => {
            ctx.is_global(&c.callee, "Boolean")
                && c.arguments.first().is_some_and(|a| a.span() == span)
        }
        AstKind::NewExpression(n) => {
            ctx.is_global(&n.callee, "Boolean")
                && n.arguments.first().is_some_and(|a| a.span() == span)
        }
        _ => false,
    }
}

impl JsRule for NoExtraBooleanCast {
    fn meta(&self) -> &'static RuleMeta {
        &NO_EXTRA_BOOLEAN_CAST
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::UnaryExpression, AstType::CallExpression]
    }

    fn callee_names(&self) -> &'static [&'static str] {
        &["Boolean"]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let message = match node.kind() {
            AstKind::UnaryExpression(u) => {
                if u.operator != UnaryOperator::LogicalNot {
                    return;
                }
                let Some(inner) = is_not(&u.argument) else {
                    return;
                };
                // `!!!x`: the inner `!!` is reported (its parent `!` is the context).
                if is_not(inner).is_some() {
                    return;
                }
                "Remove the redundant '!!': this context already converts the value to a boolean"
            }
            AstKind::CallExpression(call) => {
                if call.optional
                    || call.arguments.len() != 1
                    || matches!(call.arguments[0], Argument::SpreadElement(_))
                    || !ctx.is_global(&call.callee, "Boolean")
                {
                    return;
                }
                "Remove the redundant 'Boolean' call: this context already converts the value to a boolean"
            }
            _ => return,
        };
        let Some((parent, span)) = util::parent_through_wrappers(ctx, node) else {
            return;
        };
        if in_boolean_context(ctx, parent, span) {
            ctx.emit(node.span(), message);
        }
    }
}
