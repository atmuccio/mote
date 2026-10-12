//! `java/identical-branches` — `if/else` or `?:` whose two branches are the same code.

use super::util::{self, Fld, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Medium,
    tags: &["suspicious", "copy-paste"],
    remediation_minutes: 5,
    description: r#"Detects an `if` statement whose `then` and `else` branches contain the same
code, and a conditional expression `c ? x : x` whose two results are the same.

**Why it matters.** The condition has no effect, which is either a copy-paste mistake (one
branch was meant to do something else) or leftover code that makes readers look for a
difference that isn't there.

**How to fix.** Fix the branch that should differ, or drop the condition and keep one copy of
the code.

```java
// Noncompliant
if (user.isAdmin()) {
    render(page);
} else {
    render(page);
}
String label = compact ? name : name;

// Compliant
render(page);
String label = compact ? shortName : name;
```

Branches are compared ignoring whitespace and comments. `else if` chains are checked link by
link: only an `if` whose `else` is a plain block or statement is compared. Two empty blocks are
left to `empty-block`."#,
    ..RuleMeta::new(
        "java/identical-branches",
        "java",
        "Both branches of a conditional are identical",
    )
};

pub struct IdenticalBranches;

impl<'t> JavaRule<'t> for IdenticalBranches {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["if_statement", "ternary_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let is_if = kind(node) == "if_statement";
        // Both kinds name their branches `consequence` / `alternative`.
        let (Some(then), Some(alt)) = (node.fld(F::Consequence), node.fld(F::Alternative)) else {
            return;
        };
        if is_if {
            if kind(alt) == "if_statement" {
                return;
            }
            if kind(then) == "block" && syntax::is_empty_block(then) {
                return;
            }
        }
        if !util::same_code(then, alt, ctx.source()) {
            return;
        }
        if is_if {
            let at = util::keyword(node, "if").unwrap_or(node);
            let else_kw = util::keyword(node, "else").unwrap_or(alt);
            ctx.emit(
                at,
                "Remove this condition or fix one branch; both branches execute the same code",
            )
            .secondary(else_kw, "identical else branch");
        } else {
            ctx.emit(
                node,
                "Remove this condition or fix one branch; both results are the same expression",
            );
        }
    }
}
