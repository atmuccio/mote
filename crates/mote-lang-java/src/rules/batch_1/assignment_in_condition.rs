//! `java/assignment-in-condition` — `if (a = b)`, `while (done = check())`.

use super::util::{self, Fld, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Medium,
    tags: &["suspicious", "confusing"],
    remediation_minutes: 5,
    description: r#"Detects an `if`, `while`, or `do … while` whose whole condition is an
assignment, such as `if (found = true)`.

**Why it matters.** For `boolean` variables the compiler accepts `=` where `==` was almost
certainly meant, and the condition then always takes the assigned value. Even when the
assignment is deliberate, readers can't tell it apart from that typo.

**How to fix.** Use `==` if a comparison was meant; otherwise assign first and test the variable
on its own.

```java
// Noncompliant
if (ready = isReady()) { start(); }

// Compliant
ready = isReady();
if (ready) { start(); }
```

An assignment that is only part of the condition — the reading-loop idiom
`while ((line = reader.readLine()) != null)` — is not reported, and neither is an assignment
wrapped in an extra pair of parentheses (`if ((ok = retry()))`), which marks it as
intentional."#,
    ..RuleMeta::new(
        "java/assignment-in-condition",
        "java",
        "Assignment used as a condition",
    )
};

pub struct AssignmentInCondition;

impl<'t> JavaRule<'t> for AssignmentInCondition {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["if_statement", "while_statement", "do_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(cond) = node.fld(F::Condition) else {
            return;
        };
        // `condition` is the parenthesized expression of the statement syntax itself; a second
        // pair of parentheses is an explicit "yes, I mean it".
        let inner = if kind(cond) == "parenthesized_expression" {
            match syntax::first_named_child(cond) {
                Some(i) => i,
                None => return,
            }
        } else {
            cond
        };
        if kind(inner) != "assignment_expression" {
            return;
        }
        let at = util::op(inner).unwrap_or(inner);
        let msg = if util::op_str(inner) == "=" {
            "Use `==` to compare, or move this assignment out of the condition"
        } else {
            "Move this assignment out of the condition"
        };
        ctx.emit(at, msg);
    }
}
