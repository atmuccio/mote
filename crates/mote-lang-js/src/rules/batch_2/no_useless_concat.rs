//! `js/no-useless-concat`

use super::util;
use crate::prelude::*;

pub struct NoUselessConcat;

pub static NO_USELESS_CONCAT: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["redundant"],
    remediation_minutes: 1,
    description: r#"Flags `+` between two string (or template) literals on the same line.

**Why:** joining two literals produces a constant the author could simply have written as one
literal; the split makes the text harder to read and to search for.

**Fix:** merge the literals into one. Concatenations that span lines (a long text wrapped over
several lines) are not reported.

```js
const greeting = 'Hello, ' + 'world';   // flagged
const greeting = 'Hello, world';        // compliant
const long = 'first part of a long sentence ' +
  'continued on the next line';         // compliant
```
"#,
    ..RuleMeta::new(
        "js/no-useless-concat",
        "js",
        "String literals should not be concatenated on the same line",
    )
};

fn addition<'b, 'a>(expr: &'b Expression<'a>) -> Option<&'b BinaryExpression<'a>> {
    match helpers::unwrap_expr(expr) {
        Expression::BinaryExpression(b) if b.operator == BinaryOperator::Addition => Some(b),
        _ => None,
    }
}

impl JsRule for NoUselessConcat {
    fn meta(&self) -> &'static RuleMeta {
        &NO_USELESS_CONCAT
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::BinaryExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::BinaryExpression(expr) = node.kind() else {
            return;
        };
        if expr.operator != BinaryOperator::Addition {
            return;
        }
        // The operands that end up adjacent: the rightmost of the left side, the leftmost of the
        // right side (`x + 'a' + 'b'` joins `'a'` and `'b'`).
        let mut left = &expr.left;
        while let Some(b) = addition(left) {
            left = &b.right;
        }
        let mut right = &expr.right;
        while let Some(b) = addition(right) {
            right = &b.left;
        }
        if !util::is_string_literal(left) || !util::is_string_literal(right) {
            return;
        }
        let (left_end, right_start) = (left.span().end, right.span().start);
        if ctx.line_of(left_end) != ctx.line_of(right_start) {
            return;
        }
        let span = util::find_token(ctx, Span::new(left_end, right_start), "+")
            .unwrap_or(Span::new(left.span().start, right.span().end));
        ctx.emit(span, "Merge these two string literals into one");
    }
}
