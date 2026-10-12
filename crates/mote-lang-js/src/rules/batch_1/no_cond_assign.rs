//! `js/no-cond-assign`

use super::util;
use crate::prelude::*;

pub struct NoCondAssign;

pub static NO_COND_ASSIGN: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["suspicious"],
    remediation_minutes: 2,
    description: r#"Flags an assignment used directly as the condition of an `if`, `while`, `do … while`,
`for`, or `? :` expression, unless it is wrapped in an extra pair of parentheses.

**Why:** `if (a = b)` looks almost exactly like `if (a == b)`. When a comparison was meant, the
code overwrites `a` and tests the assigned value instead — a bug that is easy to type and hard to
spot.

**Fix:** use `===` if a comparison was meant. If the assignment is intentional (a common idiom
when reading in a loop), wrap it in an extra pair of parentheses, or compare the result
explicitly.

```js
if (user = currentUser) {}                 // flagged
if (user === currentUser) {}               // compliant

while ((match = re.exec(text))) {}         // compliant: extra parentheses mark intent
while ((line = reader.next()) !== null) {} // compliant: explicit comparison
```

In a `for` header a single pair of parentheses is enough, since the header itself uses none.
"#,
    ..RuleMeta::new(
        "js/no-cond-assign",
        "js",
        "Assignments should not be used as conditions",
    )
};

impl JsRule for NoCondAssign {
    fn meta(&self) -> &'static RuleMeta {
        &NO_COND_ASSIGN
    }

    fn types(&self) -> &'static [AstType] {
        &[
            AstType::IfStatement,
            AstType::WhileStatement,
            AstType::DoWhileStatement,
            AstType::ForStatement,
            AstType::ConditionalExpression,
        ]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        // (condition, parentheses needed to show intent)
        let (test, needed) = match node.kind() {
            AstKind::IfStatement(s) => (&s.test, 2),
            AstKind::WhileStatement(s) => (&s.test, 2),
            AstKind::DoWhileStatement(s) => (&s.test, 2),
            AstKind::ForStatement(s) => match &s.test {
                Some(test) => (test, 1),
                None => return,
            },
            AstKind::ConditionalExpression(c) => (&c.test, 2),
            _ => return,
        };
        let Expression::AssignmentExpression(assign) = test else {
            return;
        };
        if util::open_parens_before(ctx, assign.span.start) >= needed {
            return;
        }
        ctx.emit(
            assign.span,
            "Use '===' to compare, or wrap this assignment in extra parentheses if it is intended",
        );
    }
}
