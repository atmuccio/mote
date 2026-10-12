//! `js/useless-increment`

use crate::prelude::*;

pub struct UselessIncrement;

pub static USELESS_INCREMENT: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["suspicious"],
    remediation_minutes: 2,
    description: r#"Flags a variable assigned its own post-increment or post-decrement: `i = i++` or
`this.count = this.count--`.

**Why:** `i++` increments `i` but evaluates to the *old* value, which the assignment then writes
back. The net effect is that `i` never changes, which is certainly not what the author wanted — a
counter that never advances or a loop that never ends.

**Fix:** write just `i++` (or `i += 1`), or `i = i + 1`.

```js
count = count++;      // flagged: count stays the same
count++;              // compliant
next = count++;       // compliant: a different variable receives the old value
```
"#,
    ..RuleMeta::new(
        "js/useless-increment",
        "js",
        "A variable should not be assigned its own post-increment",
    )
};

impl JsRule for UselessIncrement {
    fn meta(&self) -> &'static RuleMeta {
        &USELESS_INCREMENT
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.file_contains(AstType::UpdateExpression)
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::AssignmentExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::AssignmentExpression(assign) = node.kind() else {
            return;
        };
        if assign.operator != AssignmentOperator::Assign {
            return;
        }
        let Expression::UpdateExpression(update) = helpers::unwrap_expr(&assign.right) else {
            return;
        };
        if update.prefix {
            return;
        }
        let target = compact(ctx.text(assign.left.span()));
        if target.is_empty() || target != compact(ctx.text(update.argument.span())) {
            return;
        }
        if !target
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '$' | '.' | '#'))
        {
            return;
        }
        let op = update.operator.as_str();
        ctx.emit(
            assign.span,
            format!("Write '{target}{op}' alone: assigning the old value back leaves '{target}' unchanged"),
        );
    }
}

/// The source text without whitespace.
fn compact(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}
