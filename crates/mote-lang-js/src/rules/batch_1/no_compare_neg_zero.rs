//! `js/no-compare-neg-zero`

use crate::prelude::*;

pub struct NoCompareNegZero;

pub static NO_COMPARE_NEG_ZERO: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["suspicious"],
    remediation_minutes: 2,
    description: r#"Flags comparisons with negative zero, such as `x === -0` or `x < -0`.

**Why:** comparison operators treat `-0` and `+0` as the same value, so `x === -0` is also true
for `0`. Code that compares with `-0` almost always means to detect negative zero specifically,
which this comparison cannot do.

**Fix:** use `Object.is(x, -0)` to detect negative zero, or compare with `0` if the sign does not
matter.

```js
if (x === -0) {}          // flagged: also true for 0
if (Object.is(x, -0)) {}  // compliant
if (x === 0) {}           // compliant
```
"#,
    ..RuleMeta::new(
        "js/no-compare-neg-zero",
        "js",
        "Comparisons with '-0' should use 'Object.is'",
    )
};

impl JsRule for NoCompareNegZero {
    fn meta(&self) -> &'static RuleMeta {
        &NO_COMPARE_NEG_ZERO
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
        if is_negative_zero(&expr.left) || is_negative_zero(&expr.right) {
            ctx.emit(
                expr.span,
                "Use 'Object.is(x, -0)' to detect negative zero: this comparison also matches 0",
            );
        }
    }
}

fn is_negative_zero(expr: &Expression<'_>) -> bool {
    match helpers::unwrap_expr(expr) {
        Expression::UnaryExpression(u) if u.operator == UnaryOperator::UnaryNegation => {
            matches!(helpers::unwrap_expr(&u.argument), Expression::NumericLiteral(n) if n.value == 0.0)
        }
        _ => false,
    }
}
