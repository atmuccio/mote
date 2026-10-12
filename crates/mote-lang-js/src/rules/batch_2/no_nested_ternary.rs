//! `js/no-nested-ternary`

use crate::prelude::*;

pub struct NoNestedTernary;

pub static NO_NESTED_TERNARY: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["readability"],
    remediation_minutes: 5,
    description: r#"Flags a conditional (ternary) expression used directly as the condition or a
branch of another conditional expression.

**Why:** chained `?:` operators force the reader to track several conditions and their pairing
at once; precedence and indentation make it easy to misread which value belongs to which case.

**Fix:** use `if`/`else` statements, a `switch`, a lookup table, or extract the inner choice into
a well-named variable or function. Each group of nested ternaries is reported once, at the first
nested one.

```js
const label = n === 0 ? 'none' : n === 1 ? 'one' : 'many';   // flagged

function labelFor(n) {                                     // compliant
  if (n === 0) return 'none';
  return n === 1 ? 'one' : 'many';
}
```
"#,
    ..RuleMeta::new(
        "js/no-nested-ternary",
        "js",
        "Ternary operators should not be nested",
    )
};

fn nested<'b, 'a>(expr: &'b Expression<'a>) -> Option<&'b ConditionalExpression<'a>> {
    match helpers::unwrap_expr(expr) {
        Expression::ConditionalExpression(c) => Some(c),
        _ => None,
    }
}

impl JsRule for NoNestedTernary {
    fn meta(&self) -> &'static RuleMeta {
        &NO_NESTED_TERNARY
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::ConditionalExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::ConditionalExpression(cond) = node.kind() else {
            return;
        };
        let inner = nested(&cond.test)
            .or_else(|| nested(&cond.consequent))
            .or_else(|| nested(&cond.alternate));
        let Some(inner) = inner else {
            return;
        };
        // Report each nest once: only from its outermost ternary.
        if matches!(
            ctx.outer_parent(node).map(AstNode::kind),
            Some(AstKind::ConditionalExpression(_))
        ) {
            return;
        }
        ctx.emit(
            inner.span,
            "Extract this nested ternary into a separate statement or variable",
        );
    }
}
