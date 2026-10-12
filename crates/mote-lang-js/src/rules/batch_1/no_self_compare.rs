//! `js/no-self-compare`

use super::util;
use crate::prelude::*;

pub struct NoSelfCompare;

pub static NO_SELF_COMPARE: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["suspicious"],
    remediation_minutes: 5,
    description: r#"Flags comparisons whose two operands are the same side-effect-free expression, such as
`x === x`, `a.length < a.length`, or `item.id !== item.id`.

**Why:** such a comparison always gives the same answer (the only exception is `NaN`, which is
not equal to itself). It is almost always a typo where one side should name a different variable
— `a.id === b.id` written as `a.id === a.id`.

**Fix:** compare against the intended value. To test for `NaN`, use `Number.isNaN(x)`, which says
what it means.

```js
if (user.id === user.id) {}        // flagged
if (user.id === other.id) {}       // compliant
if (x !== x) {}                    // flagged: use Number.isNaN(x)
```
"#,
    ..RuleMeta::new(
        "js/no-self-compare",
        "js",
        "Values should not be compared with themselves",
    )
};

impl JsRule for NoSelfCompare {
    fn meta(&self) -> &'static RuleMeta {
        &NO_SELF_COMPARE
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::BinaryExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::BinaryExpression(expr) = node.kind() else {
            return;
        };
        if !helpers::is_comparison(expr.operator) {
            return;
        }
        // Two identical literals (`'5' === '5'`) are a constant, not a typo for another variable.
        if !util::same_pure_expr(&expr.left, &expr.right) || !mentions_variable(&expr.left) {
            return;
        }
        let operand = util::excerpt(ctx.text(expr.left.span()));
        let message = if expr.operator.is_equality() {
            format!(
                "Compare '{operand}' with the intended value, not itself (use 'Number.isNaN()' to test for NaN)"
            )
        } else {
            format!("Compare '{operand}' with the intended value, not itself")
        };
        ctx.emit(expr.span, message);
    }
}

/// Does the expression read a variable, `this`, or a property?
fn mentions_variable(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::Identifier(_)
        | Expression::ThisExpression(_)
        | Expression::StaticMemberExpression(_)
        | Expression::ComputedMemberExpression(_)
        | Expression::PrivateFieldExpression(_)
        | Expression::ChainExpression(_) => true,
        Expression::TemplateLiteral(t) => t.expressions.iter().any(mentions_variable),
        Expression::UnaryExpression(u) => mentions_variable(&u.argument),
        Expression::BinaryExpression(b) => {
            mentions_variable(&b.left) || mentions_variable(&b.right)
        }
        Expression::LogicalExpression(l) => {
            mentions_variable(&l.left) || mentions_variable(&l.right)
        }
        Expression::ConditionalExpression(c) => {
            mentions_variable(&c.test)
                || mentions_variable(&c.consequent)
                || mentions_variable(&c.alternate)
        }
        Expression::TSAsExpression(e) => mentions_variable(&e.expression),
        Expression::TSSatisfiesExpression(e) => mentions_variable(&e.expression),
        Expression::TSNonNullExpression(e) => mentions_variable(&e.expression),
        Expression::TSTypeAssertion(e) => mentions_variable(&e.expression),
        _ => false,
    }
}
