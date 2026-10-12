//! `js/bitwise-on-booleans`

use super::util;
use crate::prelude::*;

pub struct BitwiseOnBooleans;

pub static BITWISE_ON_BOOLEANS: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["suspicious", "typo"],
    remediation_minutes: 2,
    description: r#"Flags the bitwise operators `&` and `|` applied to two boolean expressions, such as
`a > 0 & b > 0` or `!done | failed === true`.

**Why:** with two comparisons on either side, `&` and `|` are almost always a typo for `&&` and
`||`. The result is the number `0` or `1` rather than `true`/`false` (so `=== true` checks and
TypeScript's types disagree), both sides are always evaluated, and the precedence differs from the
logical operators, which makes mixed expressions evaluate in a surprising order.

**Fix:** use `&&` or `||`.

```js
if (x > 0 & y > 0) {}          // flagged
if (isOpen | count === 0) {}   // not reported: `isOpen` is not visibly a boolean

if (x > 0 && y > 0) {}         // compliant
const bits = flags & MASK;     // compliant: numbers
```

Only operands that are visibly booleans are considered: comparisons, `!x`, `instanceof`, `in`,
boolean literals, and `&&`/`||` combinations of those. `^` is not reported (it is a common way to
write "exactly one of" for booleans), nor are the compound forms `|=` and `&=`.
"#,
    ..RuleMeta::new(
        "js/bitwise-on-booleans",
        "js",
        "Bitwise operators should not be applied to booleans",
    )
};

impl JsRule for BitwiseOnBooleans {
    fn meta(&self) -> &'static RuleMeta {
        &BITWISE_ON_BOOLEANS
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::BinaryExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::BinaryExpression(expr) = node.kind() else {
            return;
        };
        let (op, logical) = match expr.operator {
            BinaryOperator::BitwiseAnd => ("&", "&&"),
            BinaryOperator::BitwiseOR => ("|", "||"),
            _ => return,
        };
        if !is_boolean(&expr.left) || !is_boolean(&expr.right) {
            return;
        }
        let span = util::find_in(ctx, expr.left.span().end, expr.right.span().start, op)
            .unwrap_or(expr.span);
        ctx.emit(
            span,
            format!("Use '{logical}' instead of '{op}': both operands are booleans"),
        );
    }
}

/// Is the expression visibly a boolean?
fn is_boolean(expr: &Expression<'_>) -> bool {
    match helpers::unwrap_expr(expr) {
        Expression::BooleanLiteral(_) => true,
        Expression::UnaryExpression(u) => u.operator == UnaryOperator::LogicalNot,
        Expression::BinaryExpression(b) => {
            b.operator.is_equality()
                || b.operator.is_compare()
                || matches!(b.operator, BinaryOperator::In | BinaryOperator::Instanceof)
        }
        Expression::PrivateInExpression(_) => true,
        Expression::LogicalExpression(l) => {
            l.operator != LogicalOperator::Coalesce && is_boolean(&l.left) && is_boolean(&l.right)
        }
        _ => false,
    }
}
