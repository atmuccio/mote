//! `js/no-self-compare`

use crate::prelude::*;

pub struct NoSelfCompare;

pub static NO_SELF_COMPARE: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,           // Bug | CodeSmell (default CodeSmell)
    quality: Quality::Reliability, // Reliability | Maintainability (default)
    severity: Severity::High,      // see section 8
    tags: &["suspicious"],
    remediation_minutes: 5,
    params: &[], // see "Parameters" below
    description: r#"Flags comparisons of an expression with itself (`x === x`).

**Why:** the result is always the same (except for `NaN`), so the comparison is almost certainly a
typo for another variable.

**Fix:** compare against the intended value; use `Number.isNaN(x)` to test for `NaN`.

```js
if (a === a) {}          // flagged
if (a === b) {}          // compliant
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
        let (left, right) = (
            helpers::unwrap_expr(&expr.left),
            helpers::unwrap_expr(&expr.right),
        );
        if let (Expression::Identifier(a), Expression::Identifier(b)) = (left, right) {
            if a.name == b.name {
                ctx.emit(
                    expr.span,
                    format!("Compare '{}' with the intended value, not itself", a.name),
                );
            }
        }
    }
}
