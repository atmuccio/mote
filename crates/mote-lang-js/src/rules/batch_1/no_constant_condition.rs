//! `js/no-constant-condition`

use crate::prelude::*;

pub struct NoConstantCondition;

pub static NO_CONSTANT_CONDITION: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["suspicious"],
    remediation_minutes: 5,
    params: &[ParamSpec {
        name: "check_loops",
        default: ParamDefault::Bool(false),
        description: "Also check loop conditions (`while (true)`, `do … while (1)`).",
    }],
    description: r#"Flags `if` statements and `? :` expressions whose condition is a constant — a literal, an
object or function literal, `typeof x`, `void x`, `undefined`, or an operator applied only to such
values.

**Why:** a constant condition always takes the same branch, so the other branch is dead code.
This is usually left-over debugging (`if (false)`), a typo, or a mistaken test such as
`if (x || {})` or `if (typeof x)`, which are always true.

**Fix:** use the intended condition, or remove the dead branch and the test.

```js
if (false) { debugMode(); }      // flagged
if ([]) { handle(); }            // flagged: an array is always truthy
const v = 0 ? a : b;             // flagged
if (config.debug) { debugMode(); } // compliant
```

Loop conditions such as `while (true)` are idiomatic infinite loops and are only checked when the
`check_loops` parameter is enabled.
"#,
    ..RuleMeta::new(
        "js/no-constant-condition",
        "js",
        "Conditions should not be constant",
    )
};

impl JsRule for NoConstantCondition {
    fn meta(&self) -> &'static RuleMeta {
        &NO_CONSTANT_CONDITION
    }

    fn types(&self) -> &'static [AstType] {
        &[
            AstType::IfStatement,
            AstType::ConditionalExpression,
            AstType::WhileStatement,
            AstType::DoWhileStatement,
            AstType::ForStatement,
        ]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let (test, is_loop) = match node.kind() {
            AstKind::IfStatement(s) => (&s.test, false),
            AstKind::ConditionalExpression(c) => (&c.test, false),
            AstKind::WhileStatement(s) => (&s.test, true),
            AstKind::DoWhileStatement(s) => (&s.test, true),
            AstKind::ForStatement(s) => match &s.test {
                Some(test) => (test, true),
                None => return,
            },
            _ => return,
        };
        if !is_constant(ctx, test) {
            return;
        }
        if is_loop && !ctx.param_bool("check_loops") {
            return;
        }
        ctx.emit(
            test.span(),
            "Replace this constant condition: the same branch is always taken",
        );
    }
}

/// Is the truthiness of `expr` known without running the program?
fn is_constant(ctx: &Ctx<'_>, expr: &Expression<'_>) -> bool {
    match helpers::unwrap_expr(expr) {
        Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BigIntLiteral(_)
        | Expression::StringLiteral(_)
        | Expression::RegExpLiteral(_)
        | Expression::ArrayExpression(_)
        | Expression::ObjectExpression(_)
        | Expression::FunctionExpression(_)
        | Expression::ArrowFunctionExpression(_)
        | Expression::ClassExpression(_) => true,
        // Truthy as soon as one literal part is non-empty; otherwise it depends on the values.
        Expression::TemplateLiteral(t) => {
            t.expressions.is_empty()
                || t.quasis
                    .iter()
                    .any(|q| q.value.cooked.is_some_and(|c| !c.is_empty()))
        }
        Expression::Identifier(id) => {
            matches!(id.name.as_str(), "undefined" | "NaN" | "Infinity")
                && ctx.is_global_reference(id)
        }
        Expression::UnaryExpression(u) => match u.operator {
            UnaryOperator::Void | UnaryOperator::Typeof => true,
            UnaryOperator::LogicalNot
            | UnaryOperator::UnaryNegation
            | UnaryOperator::UnaryPlus
            | UnaryOperator::BitwiseNot => is_constant(ctx, &u.argument),
            UnaryOperator::Delete => false,
        },
        Expression::BinaryExpression(b) => {
            is_constant_value(&b.left) && is_constant_value(&b.right)
        }
        Expression::LogicalExpression(l) => is_constant(ctx, &l.left) && is_constant(ctx, &l.right),
        Expression::SequenceExpression(s) => {
            s.expressions.last().is_some_and(|e| is_constant(ctx, e))
        }
        _ => false,
    }
}

/// A primitive literal (possibly negated), whose value — not only its truthiness — is fixed.
fn is_constant_value(expr: &Expression<'_>) -> bool {
    match helpers::unwrap_expr(expr) {
        Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BigIntLiteral(_)
        | Expression::StringLiteral(_) => true,
        Expression::TemplateLiteral(t) => t.expressions.is_empty(),
        Expression::UnaryExpression(u) => {
            matches!(
                u.operator,
                UnaryOperator::LogicalNot | UnaryOperator::UnaryNegation | UnaryOperator::UnaryPlus
            ) && is_constant_value(&u.argument)
        }
        _ => false,
    }
}
