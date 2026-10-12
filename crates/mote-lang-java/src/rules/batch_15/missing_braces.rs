//! `java/missing-braces` — an `if`, `else`, `for`, `while` or `do` whose body is a single
//! statement without braces.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["conventions", "braces"],
    remediation_minutes: 1,
    description: r#"Detects `if`, `else`, `for`, `while` and `do` statements whose body is a
single statement written without braces.

**Why it matters.** A body without braces ends after its first statement, whatever the
indentation says. A second statement added later "inside" the `if` runs unconditionally, and a
debugging line slipped in before the body silently becomes the body. Braces make the extent of
every body explicit, and a code base that always uses them never has to wonder. Formatters
re-indent code but do not add the braces.

**How to fix.** Put the body in a block, even when it is one statement.

```java
// Noncompliant
if (items.isEmpty())
    return Collections.emptyList();
for (Item item : items) total += item.price();

// Compliant
if (items.isEmpty()) {
    return Collections.emptyList();
}
for (Item item : items) {
    total += item.price();
}
```

Not reported: `else if` chains (the nested `if` is checked itself), and an empty body written
as a lone `;` (see `java/empty-statement`). The rule is off by default: it is part of the
`conventions` profile. Layouts that misstate the control flow are reported by
`java/misleading-indentation` and `java/misleading-if-layout`, which are on by default."#,
    ..RuleMeta::new(
        "java/missing-braces",
        "java",
        "Control statement body without braces",
    )
};

pub struct MissingBraces;

/// Whether a body needs braces: a statement that is not a block. A missing body, or an empty
/// statement (`;`, an anonymous token), is left to other rules.
fn unbraced(body: Option<Node<'_>>) -> bool {
    body.is_some_and(|b| b.is_named() && kind(b) != "block")
}

impl<'t> JavaRule<'t> for MissingBraces {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "enhanced_for_statement",
            "while_statement",
            "do_statement",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let (body, kw) = match kind(node) {
            "if_statement" => (node.field(Field::Consequence), "if"),
            "for_statement" | "enhanced_for_statement" => (node.field(Field::Body), "for"),
            "while_statement" => (node.field(Field::Body), "while"),
            "do_statement" => (node.field(Field::Body), "do"),
            _ => return,
        };
        if unbraced(body) {
            let at = node
                .child(0)
                .filter(|c| !c.is_named() && kind(*c) == kw)
                .unwrap_or(node);
            ctx.emit(at, format!("Put the body of this `{kw}` in braces"));
        }
        if kw != "if" {
            return;
        }
        let Some(alt) = node.field(Field::Alternative) else {
            return;
        };
        if kind(alt) != "if_statement" && unbraced(Some(alt)) {
            let at = util::keyword(node, "else").unwrap_or(alt);
            ctx.emit(at, "Put the body of this `else` in braces");
        }
    }
}
