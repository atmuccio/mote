//! `java/colon-switch-expression` — a switch expression written with `case …:` groups and
//! `yield` instead of arrow labels.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Incubating,
    default_enabled: false,
    tags: &["conventions", "switch"],
    remediation_minutes: 2,
    description: r#"Detects `switch` expressions (a `switch` that produces a value) whose cases
use the old colon form, `case X:` followed by statements and `yield`, instead of arrow labels.

**Why it matters.** The colon form keeps the fall-through semantics of the classic switch
statement: a group without `yield` or `throw` runs into the next one, which is exactly the
mistake switch expressions were designed to rule out. Arrow labels never fall through, need no
`yield` for a single expression, and make each case visibly self-contained.

**How to fix.** Rewrite each group as `case X -> value;` (or `case X -> { …; yield value; }`
when it needs statements).

```java
// Noncompliant
int days = switch (month) {
    case FEBRUARY:
        yield 28;
    case APRIL, JUNE, SEPTEMBER, NOVEMBER:
        yield 30;
    default:
        yield 31;
};

// Compliant
int days = switch (month) {
    case FEBRUARY -> 28;
    case APRIL, JUNE, SEPTEMBER, NOVEMBER -> 30;
    default -> 31;
};
```

Not reported: `switch` statements, which may use either form. The rule is off by default: it
is part of the `conventions` profile."#,
    ..RuleMeta::new(
        "java/colon-switch-expression",
        "java",
        "Switch expression with colon labels",
    )
};

pub struct ColonSwitchExpression;

/// Parents under which a `switch` is a statement rather than an expression.
fn is_statement_position(parent: Node<'_>) -> bool {
    matches!(
        kind(parent),
        "block"
            | "constructor_body"
            | "switch_block_statement_group"
            | "switch_rule"
            | "labeled_statement"
            | "if_statement"
            | "while_statement"
            | "for_statement"
            | "enhanced_for_statement"
            | "do_statement"
            | "expression_statement"
            | "program"
            | "ERROR"
    )
}

impl<'t> JavaRule<'t> for ColonSwitchExpression {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["switch_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        if !syntax::named_children(body).any(|c| kind(c) == "switch_block_statement_group") {
            return;
        }
        if ctx.parent(node).is_none_or(is_statement_position) {
            return;
        }
        let at = node
            .child(0)
            .filter(|c| !c.is_named() && kind(*c) == "switch")
            .unwrap_or(node);
        ctx.emit(
            at,
            "Use arrow labels (`case … ->`) in this switch expression instead of `:` and `yield`",
        );
    }
}
