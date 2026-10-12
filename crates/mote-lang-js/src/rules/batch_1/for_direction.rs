//! `js/for-direction`

use crate::prelude::*;

pub struct ForDirection;

pub static FOR_DIRECTION: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["suspicious"],
    remediation_minutes: 5,
    description: r#"Flags `for` loops whose counter moves away from the bound in the loop condition, such as
`for (let i = 0; i < n; i--)`.

**Why:** if the counter moves in the wrong direction the condition stays true, so the loop runs
until the counter overflows to `-Infinity`/`Infinity` or never ends; if the condition starts out
false the loop never runs. Either way the `++`/`--` (or `+=`/`-=`) was almost certainly
mistyped.

**Fix:** make the update move the counter toward the bound (`i++` for `<`/`<=`, `i--` for
`>`/`>=`), or fix the comparison.

```js
for (let i = 0; i < items.length; i--) {}      // flagged
for (let i = items.length - 1; i >= 0; i++) {} // flagged
for (let i = 0; i < items.length; i++) {}      // compliant
```

Only updates with a known direction are checked: `++`, `--`, and `+=`/`-=` with a numeric
literal.
"#,
    ..RuleMeta::new(
        "js/for-direction",
        "js",
        "For-loop counters should move toward the loop bound",
    )
};

impl JsRule for ForDirection {
    fn meta(&self) -> &'static RuleMeta {
        &FOR_DIRECTION
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::ForStatement]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::ForStatement(stmt) = node.kind() else {
            return;
        };
        let (Some(test), Some(update)) = (&stmt.test, &stmt.update) else {
            return;
        };
        let Some((counter, step)) = update_direction(update) else {
            return;
        };
        let Expression::BinaryExpression(cmp) = helpers::unwrap_expr(test) else {
            return;
        };
        let left = name_of(&cmp.left);
        let right = name_of(&cmp.right);
        // The direction the counter must move for the condition to become false eventually.
        let wanted = match (left == Some(counter), right == Some(counter)) {
            (true, false) => match cmp.operator {
                BinaryOperator::LessThan | BinaryOperator::LessEqualThan => 1.0,
                BinaryOperator::GreaterThan | BinaryOperator::GreaterEqualThan => -1.0,
                _ => return,
            },
            (false, true) => match cmp.operator {
                BinaryOperator::LessThan | BinaryOperator::LessEqualThan => -1.0,
                BinaryOperator::GreaterThan | BinaryOperator::GreaterEqualThan => 1.0,
                _ => return,
            },
            _ => return,
        };
        if step * wanted < 0.0 {
            ctx.emit(
                update.span(),
                format!("Change this update: '{counter}' moves away from the loop bound, so the condition never becomes false"),
            );
        }
    }
}

fn name_of<'a>(expr: &Expression<'a>) -> Option<&'a str> {
    match helpers::unwrap_expr(expr) {
        Expression::Identifier(id) => Some(id.name.as_str()),
        _ => None,
    }
}

/// The counter updated by `i++`, `--i`, `i += 2`, `i -= 1`, and the sign of the step.
fn update_direction<'a>(update: &Expression<'a>) -> Option<(&'a str, f64)> {
    match helpers::unwrap_expr(update) {
        Expression::UpdateExpression(u) => {
            let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &u.argument else {
                return None;
            };
            let step = match u.operator {
                UpdateOperator::Increment => 1.0,
                UpdateOperator::Decrement => -1.0,
            };
            Some((id.name.as_str(), step))
        }
        Expression::AssignmentExpression(a) => {
            let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else {
                return None;
            };
            let value = helpers::number_value(&a.right)?;
            if value == 0.0 || !value.is_finite() {
                return None;
            }
            let step = match a.operator {
                AssignmentOperator::Addition => value,
                AssignmentOperator::Subtraction => -value,
                _ => return None,
            };
            Some((id.name.as_str(), step.signum()))
        }
        _ => None,
    }
}
