//! `js/no-unsafe-negation`

use crate::prelude::*;

pub struct NoUnsafeNegation;

pub static NO_UNSAFE_NEGATION: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["suspicious"],
    remediation_minutes: 2,
    description: r#"Flags `!` applied to the left operand of `in` or `instanceof` without parentheses, as in
`!key in obj` or `!value instanceof Date`.

**Why:** `!` binds tighter than `in` and `instanceof`, so `!key in obj` means `(!key) in obj` —
it checks whether `"true"` or `"false"` is a key of `obj`. The author almost certainly meant
`!(key in obj)`, and the condition silently gives the wrong answer.

**Fix:** put the negation around the whole test: `!(key in obj)`. If negating the left operand
really is intended, add parentheses to say so: `(!key) in obj`.

```js
if (!key in cache) {}               // flagged
if (!(key in cache)) {}             // compliant
if (!(value instanceof Date)) {}    // compliant
```
"#,
    ..RuleMeta::new(
        "js/no-unsafe-negation",
        "js",
        "Negation should not be applied to the left operand of 'in' or 'instanceof'",
    )
};

impl JsRule for NoUnsafeNegation {
    fn meta(&self) -> &'static RuleMeta {
        &NO_UNSAFE_NEGATION
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::BinaryExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::BinaryExpression(expr) = node.kind() else {
            return;
        };
        let operator = match expr.operator {
            BinaryOperator::In => "in",
            BinaryOperator::Instanceof => "instanceof",
            _ => return,
        };
        let Expression::UnaryExpression(negation) = &expr.left else {
            return;
        };
        if negation.operator != UnaryOperator::LogicalNot {
            return;
        }
        // `(!a) in b`: the binary expression starts at the parenthesis, before the operand.
        if expr.span.start != negation.span.start {
            return;
        }
        ctx.emit(
            negation.span,
            format!("Negate the whole '{operator}' test with '!(… {operator} …)', or parenthesize the operand if negating it is intended"),
        );
    }
}
