//! `java/duplicate-condition` — the same condition twice in an `if / else if` chain.

use super::util::{self, Fld, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["suspicious", "copy-paste"],
    remediation_minutes: 5,
    description: r#"Detects an `if / else if` chain that tests the same condition more than once.

**Why it matters.** A branch is only reached when every earlier condition was false, so a
repeated condition can never be true there: its branch is dead code. This is usually a
copy-paste slip where the second test should check something else.

**How to fix.** Correct the repeated condition, or merge the branches if they really belong
together.

```java
// Noncompliant
if (type == FILE) {
    openFile();
} else if (type == DIRECTORY) {
    listDirectory();
} else if (type == FILE) {      // never true here
    openLink();
}

// Compliant
if (type == FILE) {
    openFile();
} else if (type == DIRECTORY) {
    listDirectory();
} else if (type == LINK) {
    openLink();
}
```

Conditions are compared ignoring whitespace, comments, and redundant outer parentheses.
Conditions that assign, increment, or create objects are not reported."#,
    ..RuleMeta::new(
        "java/duplicate-condition",
        "java",
        "Repeated condition in an if / else if chain",
    )
};

pub struct DuplicateCondition;

fn condition(if_stmt: Node<'_>) -> Option<Node<'_>> {
    if_stmt.fld(F::Condition)
}

/// The next `if` of the chain (`else if`).
fn next_in_chain(if_stmt: Node<'_>) -> Option<Node<'_>> {
    if_stmt
        .fld(F::Alternative)
        .filter(|a| kind(*a) == "if_statement")
}

impl<'t> JavaRule<'t> for DuplicateCondition {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["if_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        // Only the head of a chain; `else if` links are handled from there.
        if let Some(p) = ctx.parent(node) {
            if kind(p) == "if_statement"
                && p.fld(F::Alternative).is_some_and(|a| a.id() == node.id())
            {
                return;
            }
        }
        if next_in_chain(node).is_none() {
            return;
        }
        let src = ctx.source();
        let mut cur = next_in_chain(node);
        let mut steps = 0;
        while let Some(link) = cur {
            steps += 1;
            if steps > 500 {
                return;
            }
            if let Some(cond) = condition(link) {
                if !util::has_mutation(cond) {
                    // Compare with every earlier condition of the chain.
                    let mut prev = Some(node);
                    while let Some(p) = prev {
                        if p.id() == link.id() {
                            break;
                        }
                        if let Some(pc) = condition(p) {
                            if util::same_code(pc, cond, src) {
                                ctx.emit(
                                    cond,
                                    "Correct this condition; it repeats an earlier one of the chain, so its branch is dead",
                                )
                                .secondary(pc, "first tested here");
                                break;
                            }
                        }
                        prev = next_in_chain(p);
                    }
                }
            }
            cur = next_in_chain(link);
        }
    }
}
