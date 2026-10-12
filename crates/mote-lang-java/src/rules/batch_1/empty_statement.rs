//! `java/empty-statement` — stray `;` statements.

use super::util::{self, Fld, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["suspicious", "clutter"],
    remediation_minutes: 1,
    description: r#"Detects empty statements — a lone `;` — such as `foo();;`, a `;` after a
member declaration (in a class, interface or enum body), or `if (x);`.

**Why it matters.** A doubled semicolon is only clutter, but a `;` right after `if (…)`,
`while (…)` or an enhanced `for (…)` header silently becomes the whole body: the block
that follows runs unconditionally (or once), which is a real bug that is easy to miss.

**How to fix.** Delete the stray semicolon. If a loop intentionally has no body, write an
empty block with a comment instead.

```java
// Noncompliant
if (list.isEmpty());
{
    return;             // always returns
}
counter++;;

// Compliant
if (list.isEmpty()) {
    return;
}
counter++;
```

Not reported: the semicolons inside a `for (;;)` header, a classic `for` loop whose header
does the work (`for (i = 0; a[i] != 0; i++);`), and a `while`/`do` whose condition calls a
method (`while (it.next() != null);`), and the `;` that ends the constants of an enum."#,
    ..RuleMeta::new("java/empty-statement", "java", "Empty statement")
};

pub struct EmptyStatement;

fn is_semicolon(n: Node<'_>) -> bool {
    !n.is_named() && kind(n) == ";"
}

impl<'t> JavaRule<'t> for EmptyStatement {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "block",
            "constructor_body",
            "switch_block_statement_group",
            "class_body",
            "interface_body",
            "enum_body_declarations",
            "program",
            "labeled_statement",
            "if_statement",
            "while_statement",
            "do_statement",
            "enhanced_for_statement",
            "for_statement",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if node.has_error() {
            return;
        }
        match kind(node) {
            "if_statement" => {
                for f in [F::Consequence, F::Alternative] {
                    if let Some(s) = node.fld(f).filter(|s| is_semicolon(*s)) {
                        let kw = if matches!(f, F::Consequence) {
                            "if"
                        } else {
                            "else"
                        };
                        ctx.emit(
                            s,
                            format!("Remove this stray `;`; it is the whole body of the `{kw}`"),
                        );
                    }
                }
            }
            "while_statement" | "do_statement" | "enhanced_for_statement" | "for_statement" => {
                let Some(body) = node.fld(F::Body).filter(|s| is_semicolon(*s)) else {
                    return;
                };
                if util::loop_header_does_work(node) {
                    return;
                }
                ctx.emit(
                    body,
                    "Remove this stray `;`; it is the whole body of the loop",
                );
            }
            k => {
                // In statement lists and type bodies every direct `;` child is an empty
                // statement (real statements and declarations carry their own `;`).
                // Cheap pre-check: braced bodies have exactly two other anonymous children
                // (`{` and `}`), the file none; only look further when there are more.
                let expected_anonymous = match k {
                    "block" | "constructor_body" | "class_body" | "interface_body" => Some(2),
                    // The `;` that ends the list of enum constants.
                    "enum_body_declarations" => Some(1),
                    "program" => Some(0),
                    _ => None,
                };
                if let Some(expected) = expected_anonymous {
                    let anonymous =
                        (node.child_count() as usize).saturating_sub(node.named_child_count());
                    if anonymous <= expected {
                        return;
                    }
                }
                let separator = (k == "enum_body_declarations")
                    .then(|| syntax::children(node).find(|c| is_semicolon(*c)))
                    .flatten();
                for c in syntax::children(node) {
                    if is_semicolon(c) && separator.is_none_or(|sep| sep.id() != c.id()) {
                        ctx.emit(c, "Remove this empty statement");
                    }
                }
            }
        }
    }
}
