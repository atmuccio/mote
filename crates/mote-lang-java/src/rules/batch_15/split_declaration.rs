//! `java/split-declaration` — `int x;` immediately followed by `x = …;`.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["conventions", "readability"],
    remediation_minutes: 1,
    description: r#"Detects a local variable declared without a value whose very next statement
assigns it: `String name;` followed by `name = user.getName();`.

**Why it matters.** Splitting the declaration from the first value costs a line and opens a
gap in which the variable exists without a value. Declared together, the variable cannot be
read before it is set, can be `final` (or `var`) at a glance, and the type and the value sit
side by side.

**How to fix.** Give the variable its value in the declaration.

```java
// Noncompliant
String name;
name = user.getName();

// Compliant
String name = user.getName();
```

Only the statement right after the declaration is considered (comments in between do not
matter), and only a plain `=` assignment to the variable itself. Not reported: declarations of
several variables, and variables first assigned inside a `try`, an `if` or a loop, where the
split is needed. The rule is off by default: it is part of the `conventions` profile."#,
    ..RuleMeta::new(
        "java/split-declaration",
        "java",
        "Declaration separated from its first assignment",
    )
};

pub struct SplitDeclaration;

impl<'t> JavaRule<'t> for SplitDeclaration {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["local_variable_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        // Cheapest first: almost every declaration has an initializer.
        if ctx.text(node).contains('=') {
            return;
        }
        let mut declarators = syntax::declarators(node);
        let Some(declarator) = declarators.next() else {
            return;
        };
        if declarators.next().is_some() || declarator.field(Field::Value).is_some() {
            return;
        }
        let Some(parent) = ctx.parent(node) else {
            return;
        };
        if !matches!(
            kind(parent),
            "block" | "constructor_body" | "switch_block_statement_group"
        ) {
            return;
        }
        let Some(name) = declarator.field(Field::Name) else {
            return;
        };
        let Some(next) = util::next_code(parent, node) else {
            return;
        };
        if kind(next) != "expression_statement" {
            return;
        }
        let Some(assign) = syntax::first_named_child(next) else {
            return;
        };
        if kind(assign) != "assignment_expression" || syntax::operator_str(assign) != "=" {
            return;
        }
        let Some(left) = assign.field(Field::Left) else {
            return;
        };
        let var = ctx.text(name);
        if kind(left) != "identifier" || ctx.text(left) != var {
            return;
        }
        ctx.emit(name, format!("Give `{var}` its value where it is declared"))
            .secondary(left, "first assigned here");
    }
}
