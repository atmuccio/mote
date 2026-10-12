//! `java/one-variable-per-declaration` — several variables declared in one statement.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["conventions", "readability"],
    remediation_minutes: 1,
    description: r#"Detects field, constant and local variable declarations that declare more
than one variable (`int width, height;`).

**Why it matters.** One name per declaration gives every variable its own line for its type,
its initial value, its annotations and its comment. Grouped declarations hide which names are
initialized (`int a, b = 0;` leaves `a` without a value), make the type of later names easy to
misread when array brackets are mixed in (`int a, b[];` declares an `int` and an `int[]`), and
turn a change to one variable into an edit of a line that also declares others.

**How to fix.** Write one declaration per variable.

```java
// Noncompliant
private int width, height;
String first = null, last = null;

// Compliant
private int width;
private int height;
String first = null;
String last = null;
```

Not reported: the initialization part of a classic `for` loop (`for (int i = 0, n = size(); i <
n; i++)`), where several loop variables in one declaration are the usual form. The rule is off
by default: it is part of the `conventions` profile."#,
    ..RuleMeta::new(
        "java/one-variable-per-declaration",
        "java",
        "Several variables in one declaration",
    )
};

pub struct OneVariablePerDeclaration;

impl<'t> JavaRule<'t> for OneVariablePerDeclaration {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "field_declaration",
            "local_variable_declaration",
            "constant_declaration",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        // Cheapest first: a second declarator needs a comma.
        if !ctx.text(node).contains(',') {
            return;
        }
        let mut declarators = syntax::declarators(node);
        if declarators.next().is_none() {
            return;
        }
        let Some(second) = declarators.next() else {
            return;
        };
        if kind(node) == "local_variable_declaration"
            && ctx.parent(node).is_some_and(|p| kind(p) == "for_statement")
        {
            return;
        }
        let count = 2 + declarators.count();
        let at = second.field(Field::Name).unwrap_or(second);
        ctx.emit(
            at,
            format!("Split this declaration of {count} variables into one declaration each"),
        )
        .value(count as f64);
    }
}
