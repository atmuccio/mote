//! `js/negated-comparison`

use crate::prelude::*;

pub struct NegatedComparison;

pub static NEGATED_COMPARISON: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["readability"],
    remediation_minutes: 1,
    description: r#"Flags a negated equality test that the opposite operator states directly: `!(a === b)` is
`a !== b`, `!(a == b)` is `a != b`, and the reverse.

**Why:** the `!` in front of the parentheses is one more step for the reader, and it is easy to
overlook when scanning a condition. The direct operator states the condition as it is.

**Fix:** drop the `!` and the parentheses and use the opposite operator.

```js
if (!(status === 'done')) { … }     // flagged
if (status !== 'done') { … }        // compliant
```

Ordering comparisons are not reported: `!(a < b)` is not the same as `a >= b` when either side is
`NaN`, `undefined` or a value that does not convert to a number, and the rule cannot see the types.
Off by default: this is a readability preference.
"#,
    ..RuleMeta::new(
        "js/negated-comparison",
        "js",
        "Negated equality tests should use the opposite operator",
    )
};

impl JsRule for NegatedComparison {
    fn meta(&self) -> &'static RuleMeta {
        &NEGATED_COMPARISON
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::UnaryExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::UnaryExpression(unary) = node.kind() else {
            return;
        };
        if unary.operator != UnaryOperator::LogicalNot {
            return;
        }
        let Expression::BinaryExpression(binary) = &unary.argument else {
            return;
        };
        let Some(opposite) = helpers::negate_equality(binary.operator) else {
            return;
        };
        // `!!(a === b)` is a boolean cast, not a negated test.
        if matches!(ctx.parent_kind(node), Some(AstKind::UnaryExpression(u)) if u.operator == UnaryOperator::LogicalNot)
        {
            return;
        }
        ctx.emit(
            Span::new(unary.span.start, unary.span.start + 1),
            format!(
                "Use '{}' instead of negating '{}'",
                opposite.as_str(),
                binary.operator.as_str()
            ),
        );
    }
}
