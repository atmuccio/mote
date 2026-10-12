//! `js/no-return-assign`

use super::util;
use crate::prelude::*;

pub struct NoReturnAssign;

/// Ancestors looked at for the statement holding an assignment.
const MAX_WALK: usize = 512;

pub static NO_RETURN_ASSIGN: RuleMeta = RuleMeta {
    severity: Severity::Medium,
    tags: &["confusing"],
    remediation_minutes: 2,
    description: r#"Flags assignments inside a `return` statement or in the expression body of an arrow
function, unless the assignment is wrapped in its own parentheses.

**Why:** `return a = b` reads very much like `return a === b`; the reader cannot tell whether a
comparison was intended. Returning and assigning in one step also hides a side effect.

**Fix:** assign first, then return the variable. If returning the assigned value is intended,
wrap the assignment in parentheses to make that explicit: `return (this.cache = compute())`.

```js
function reset() {
  return count = 0;                // flagged
}
items.forEach(i => total += i);    // flagged

function reset() {
  count = 0;                       // compliant
  return count;
}
items.forEach(i => (total += i));  // compliant: explicit
```
"#,
    // Stylistic and noisy on real code: opt-in (`extends: all` or enable explicitly).
    default_enabled: false,
    ..RuleMeta::new(
        "js/no-return-assign",
        "js",
        "Assignments should not be made in 'return' statements",
    )
};

impl JsRule for NoReturnAssign {
    fn meta(&self) -> &'static RuleMeta {
        &NO_RETURN_ASSIGN
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::AssignmentExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::AssignmentExpression(assign) = node.kind() else {
            return;
        };
        let mut arrow_body = false;
        let mut found = false;
        // Bounded: real code puts a few expressions between an assignment and its statement; a
        // chain of thousands of `a = a = …` made the walk from every link quadratic. Giving up
        // reports nothing.
        let mut ancestors = ctx.ancestors(node).take(MAX_WALK);
        while let Some(ancestor) = ancestors.next() {
            match ancestor.kind() {
                AstKind::ReturnStatement(_) => {
                    found = true;
                    break;
                }
                // `x => a = b`: the body is an expression statement in a `FunctionBody`.
                AstKind::ExpressionStatement(_) => {
                    let body = ancestors.next().map(AstNode::kind);
                    let arrow = ancestors.next().map(AstNode::kind);
                    if let (
                        Some(AstKind::FunctionBody(_)),
                        Some(AstKind::ArrowFunctionExpression(a)),
                    ) = (body, arrow)
                    {
                        found = a.expression;
                        arrow_body = true;
                    }
                    break;
                }
                kind if kind.is_statement()
                    || kind.is_declaration()
                    || kind.is_function_like()
                    || matches!(
                        kind,
                        AstKind::Class(_) | AstKind::FunctionBody(_) | AstKind::Program(_)
                    ) =>
                {
                    break;
                }
                _ => {}
            }
        }
        if !found || util::is_parenthesized(ctx, assign.span) {
            return;
        }
        let message = if arrow_body {
            "Move this assignment out of the arrow function's returned expression, or wrap it in parentheses"
        } else {
            "Move this assignment out of the 'return' statement, or wrap it in parentheses"
        };
        ctx.emit(assign.span, message);
    }
}
