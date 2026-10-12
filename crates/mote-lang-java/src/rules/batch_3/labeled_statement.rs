//! `java/labeled-statement` — statement labels.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["control-flow"],
    remediation_minutes: 10,
    description: r#"Detects labeled statements (`outer: for (…)`).

**Why it matters.** Labels exist to support `break label` / `continue label` jumps across
several nesting levels. That control flow is hard to follow: the reader must find the label to
know where execution continues, and the deep nesting that made the label necessary is itself a
readability problem.

**How to fix.** Extract the nested loops into a method and `return` from it, or restructure the
loop condition.

```java
// Noncompliant
search:
for (Row row : rows) {
    for (Cell cell : row) {
        if (cell.matches(query)) {
            found = cell;
            break search;
        }
    }
}

// Compliant
found = findFirstMatch(rows, query);   // the inner loops `return cell;`
```"#,
    ..RuleMeta::new("java/labeled-statement", "java", "Labeled statement")
};

pub struct LabeledStatement;

impl<'t> JavaRule<'t> for LabeledStatement {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["labeled_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(label) = named_children(node).find(|c| kind(*c) == "identifier") else {
            return;
        };
        let text = ctx.text(label);
        ctx.emit(
            label,
            format!("Remove the label `{text}`; extract the nested code into a method instead"),
        );
    }
}
