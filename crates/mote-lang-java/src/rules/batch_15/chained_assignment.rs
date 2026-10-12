//! `java/chained-assignment` — `a = b = 0;`: one statement assigning several variables.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["conventions", "readability"],
    remediation_minutes: 1,
    description: r#"Detects statements and declarations that assign one value to several
variables through a chain of assignments: `a = b = 0;`, `int x = y = size();`.

**Why it matters.** A chain reads like one assignment and hides the others: the eye stops at
the first `=`. It also binds the variables together in a way that is easy to break: whoever
later needs a different value for one of them has to untangle the chain, and with variables of
different types the outer one receives the inner one's converted value (with a `float f`,
`double d = f = 0.1;` stores 0.10000000149…). One assignment per statement keeps each
variable's value visible.

**How to fix.** Write one assignment per statement.

```java
// Noncompliant
first = last = null;
int width = height = 0;

// Compliant
first = null;
last = null;
height = 0;
int width = 0;
```

Not reported: assignments nested inside other expressions (`while ((line = in.readLine()) !=
null)`), which `java/assignment-in-operand` and `java/assignment-in-condition` cover. The rule
is off by default: it is part of the `conventions` profile."#,
    ..RuleMeta::new("java/chained-assignment", "java", "Chained assignment")
};

pub struct ChainedAssignment;

impl<'t> JavaRule<'t> for ChainedAssignment {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["assignment_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(parent) = ctx.parent(node) else {
            return;
        };
        let reported = match kind(parent) {
            // `a = b = 0;`: an assignment statement whose value is another assignment.
            "expression_statement" => {
                let Some(right) = node.field(Field::Right) else {
                    return;
                };
                if kind(syntax::strip_parens(right)) != "assignment_expression" {
                    return;
                }
                node
            }
            // `int a = b = 0;`: a declaration initialized with an assignment.
            "variable_declarator" => {
                if parent.field(Field::Value) != Some(node) {
                    return;
                }
                parent
            }
            _ => return,
        };
        let at = ctx.operator(node).unwrap_or(reported);
        ctx.emit(
            at,
            "Split this chained assignment into one assignment per variable",
        );
    }
}
