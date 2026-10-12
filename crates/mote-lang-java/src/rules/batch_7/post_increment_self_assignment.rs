//! `java/post-increment-self-assignment` — `x = x++` leaves `x` unchanged.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["suspicious", "typo"],
    remediation_minutes: 1,
    description: r#"Detects `x = x++;` and `x = x--;`: a variable assigned the value of its own
postfix increment or decrement.

**Why it matters.** A postfix `x++` yields the *old* value and then increments `x`; the
assignment runs last and stores that old value back. The increment is lost, so the statement
does nothing — a counter that never advances, a loop that never ends. The author meant
`x++` or `x = x + 1`.

**How to fix.** Drop the assignment (`x++;`) or write the new value explicitly.

```java
// Noncompliant
retries = retries++;

// Compliant
retries++;
```

`x = ++x` (prefix, which does store the new value) and assignments to a different target are
not reported."#,
    ..RuleMeta::new(
        "java/post-increment-self-assignment",
        "java",
        "Variable assigned its own post-increment",
    )
};

pub struct PostIncrementSelfAssignment;

impl<'t> JavaRule<'t> for PostIncrementSelfAssignment {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["assignment_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(right) = node.field(Field::Right) else {
            return;
        };
        let right = syntax::strip_parens(right);
        if kind(right) != "update_expression" || syntax::operator_str(node) != "=" {
            return;
        }
        let text = ctx.text(right);
        let op = if text.ends_with("++") {
            "++"
        } else if text.ends_with("--") {
            "--"
        } else {
            return;
        };
        let (Some(operand), Some(left)) =
            (syntax::first_named_child(right), node.field(Field::Left))
        else {
            return;
        };
        let operand = syntax::strip_parens(operand);
        let left = syntax::strip_parens(left);
        if !matches!(kind(left), "identifier" | "field_access")
            || kind(operand) != kind(left)
            || ctx.text(operand) != ctx.text(left)
        {
            return;
        }
        let name = ctx.text(left);
        ctx.emit(
            node,
            format!("Write `{name}{op};` alone: assigning `{name}{op}` back to `{name}` stores the old value and discards the change"),
        );
    }
}
