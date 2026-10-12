//! `java/else-after-return` — `else` after an `if` branch that always returns or throws.

use super::util::{field, FIELDS};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["readability", "nesting"],
    remediation_minutes: 2,
    description: r#"Detects an `else` block following an `if` branch that ends with `return` or
`throw`.

**Why it matters.** When the `if` branch always leaves the method, the `else` is redundant:
the code after the `if` only runs in the other case anyway. The extra block adds nesting and
hides the "early exit" structure of the method.

**How to fix.** Remove the `else` and move its content after the `if` statement.

```java
// Noncompliant
if (value == null) {
    return DEFAULT;
} else {
    return parse(value);
}

// Compliant
if (value == null) {
    return DEFAULT;
}
return parse(value);
```

Not reported: `else if` branches and `if` statements that are part of an `else if` chain,
where the symmetric form is often clearer."#,
    // Stylistic and noisy on real code: opt-in (`extends: all` or enable explicitly).
    default_enabled: false,
    ..RuleMeta::new(
        "java/else-after-return",
        "java",
        "Unnecessary else after return",
    )
};

pub struct ElseAfterReturn;

/// The last statement of a branch (the branch itself when it is not a block).
fn last_statement(branch: Node<'_>) -> Option<Node<'_>> {
    if kind(branch) == "block" {
        named_children(branch).last()
    } else {
        Some(branch)
    }
}

impl<'t> JavaRule<'t> for ElseAfterReturn {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["if_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(alt) = field(node, FIELDS.alternative) else {
            return;
        };
        if kind(alt) == "if_statement" {
            return;
        }
        // Part of an `else if` chain?
        if ctx.parent(node).is_some_and(|p| {
            kind(p) == "if_statement"
                && field(p, FIELDS.alternative).is_some_and(|a| a.id() == node.id())
        }) {
            return;
        }
        let Some(cons) = field(node, FIELDS.consequence) else {
            return;
        };
        let exits = last_statement(cons)
            .is_some_and(|s| matches!(kind(s), "return_statement" | "throw_statement"));
        if !exits {
            return;
        }
        let Some(else_kw) = syntax::children(node).find(|c| kind(*c) == "else") else {
            return;
        };
        ctx.emit(
            else_kw,
            "Remove this else: the if branch always returns or throws",
        );
    }
}
