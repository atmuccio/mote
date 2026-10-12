//! `java/collapsible-if` — `if (a) { if (b) { … } }` without `else` branches.

use super::util::{field, keyword, single_statement, FIELDS};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["readability", "nesting"],
    remediation_minutes: 2,
    description: r#"Detects an `if` statement without `else` whose only content is another `if`
statement without `else`.

**Why it matters.** Two nested conditions with nothing in between are one condition written
in two places. The extra nesting level costs indentation and attention without adding
meaning.

**How to fix.** Merge the conditions with `&&` (parenthesize them if they contain `||`).

```java
// Noncompliant
if (file != null) {
    if (file.exists()) {
        load(file);
    }
}

// Compliant
if (file != null && file.exists()) {
    load(file);
}
```

Not reported when the outer block contains a comment, which may document one of the
conditions."#,
    ..RuleMeta::new("java/collapsible-if", "java", "Collapsible if statements")
};

pub struct CollapsibleIf;

impl<'t> JavaRule<'t> for CollapsibleIf {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["if_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if field(node, FIELDS.alternative).is_some() {
            return;
        }
        let Some(body) = field(node, FIELDS.consequence) else {
            return;
        };
        if kind(body) == "block" && syntax::block_has_comment(body) {
            return;
        }
        let Some(inner) = single_statement(body) else {
            return;
        };
        if kind(inner) != "if_statement" || field(inner, FIELDS.alternative).is_some() {
            return;
        }
        ctx.emit(
            keyword(node),
            "Merge this if statement with the nested one using &&",
        );
    }
}
