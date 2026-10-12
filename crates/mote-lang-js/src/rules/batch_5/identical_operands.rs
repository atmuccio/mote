//! `js/identical-operands`

use super::util;
use crate::prelude::*;

pub struct IdenticalOperands;

pub static IDENTICAL_OPERANDS: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["suspicious", "copy-paste"],
    remediation_minutes: 5,
    description: r#"Flags logical operators, `^`, division, and remainder whose two operands are the same expression:
`a.id || a.id`, `isReady && isReady`, `size / size`, `mask ^ mask`.

**Why:** such an expression does not compute anything useful: `x || x` and `x && x` are just `x`,
`x / x` is `1`, `x % x` and `x ^ x` are `0`. Like comparing a value with itself, it is almost always a copy-paste
slip where one side should name something else (`a.id || b.id`).

**Fix:** correct the operand that was meant to differ, or simplify the expression.

```js
const name = user.nickname || user.nickname;   // flagged: probably user.fullName
if (isValid && isValid) {}                     // flagged

const name = user.nickname || user.fullName;   // compliant
```

Only side-effect-free operands (names, `this`, property accesses, and operators over those) are
compared; calls are never assumed to return the same value twice. Comparisons are covered by
`js/no-self-compare`. Not reported because they have legitimate uses: `x - x` (the finiteness test
`x - x === 0`), `x + x`, `x * x`, and `x & x` / `x | x` (a common way to truncate a number to a
32-bit integer, as in `hash = hash & hash`).
"#,
    ..RuleMeta::new(
        "js/identical-operands",
        "js",
        "Both operands of an operator should not be the same expression",
    )
};

impl JsRule for IdenticalOperands {
    fn meta(&self) -> &'static RuleMeta {
        &IDENTICAL_OPERANDS
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::BinaryExpression, AstType::LogicalExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let (left, right, op) = match node.kind() {
            AstKind::BinaryExpression(b) => {
                if !matches!(
                    b.operator,
                    BinaryOperator::BitwiseXOR
                        | BinaryOperator::Division
                        | BinaryOperator::Remainder
                ) {
                    return;
                }
                (&b.left, &b.right, b.operator.as_str())
            }
            AstKind::LogicalExpression(l) => (&l.left, &l.right, l.operator.as_str()),
            _ => return,
        };
        if !util::same_pure_expr(left, right) || !util::mentions_variable(left) {
            return;
        }
        let operand = util::excerpt(ctx.text(left.span()));
        ctx.emit(
            node.span(),
            format!("Both operands of '{op}' are '{operand}': one of them was probably meant to be different"),
        );
    }
}
