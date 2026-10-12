//! `java/multiple-unary-operators` — `!!done`, `- -x`, `~~mask`.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Reliability,
    severity: Severity::Low,
    status: RuleStatus::Incubating,
    tags: &["suspicious", "confusing"],
    remediation_minutes: 1,
    description: r#"Detects unary operators applied directly to another unary expression:
`!!ready`, `- -delta`, `~~mask`, `-~n`.

**Why it matters.** A doubled `!`, `-`, or `~` cancels itself out, so the expression is either
a roundabout way of writing the operand or — more often — a typo or an edit that went wrong
(one operator too many, a missing operand in between). Mixed stacks such as `-~n` are bit
tricks that few readers decode correctly.

**How to fix.** Remove the extra operators, or write the intended expression plainly.

```java
// Noncompliant
boolean active = !!flags.contains(ACTIVE);
int n = - -offset;

// Compliant
boolean active = flags.contains(ACTIVE);
int n = offset;
```

Not reported: increments and decrements (`--x` is an update, not two minus signs), and
stacked operators separated by parentheses (`-(-x)`), which mark the nesting as deliberate."#,
    ..RuleMeta::new(
        "java/multiple-unary-operators",
        "java",
        "Stacked unary operators",
    )
};

pub struct MultipleUnaryOperators;

impl<'t> JavaRule<'t> for MultipleUnaryOperators {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["unary_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(inner) = node.field(Field::Operand) else {
            return;
        };
        if kind(inner) != "unary_expression" {
            return;
        }
        // Report the outermost operator of a stack only once.
        if ctx
            .parent(node)
            .is_some_and(|p| kind(p) == "unary_expression")
        {
            return;
        }
        let outer_op = syntax::operator_str(node);
        let inner_op = syntax::operator_str(inner);
        if outer_op.is_empty() || inner_op.is_empty() {
            return;
        }
        let shown = if outer_op == inner_op && outer_op != "!" && outer_op != "~" {
            format!("{outer_op} {inner_op}")
        } else {
            format!("{outer_op}{inner_op}")
        };
        let (Some(a), Some(b)) = (syntax::operator(node), syntax::operator(inner)) else {
            return;
        };
        ctx.emit_range(
            Span::cover(a, b),
            format!("Remove the stacked unary operators `{shown}`: they cancel out or hide a typo"),
        );
    }
}
