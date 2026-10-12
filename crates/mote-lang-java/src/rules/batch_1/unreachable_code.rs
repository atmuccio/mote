//! `java/unreachable-code` — statements after `return`/`throw`/`break`/`continue`.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["unused", "control-flow"],
    remediation_minutes: 5,
    description: r#"Detects statements that follow a `return`, `throw`, `break`, `continue`, or
`yield` in the same block and therefore can never run.

**Why it matters.** Code that never runs is misleading: whoever wrote it expected it to have an
effect (cleanup, logging, a state change), so either the jump is misplaced or the code is dead.
The Java compiler rejects most of these, so the rule mainly catches code that is being edited
before it is compiled.

**How to fix.** Move the jump after the statements that must run, or delete the dead code.

```java
// Noncompliant
return result;
cache.clear();          // never runs

// Compliant
cache.clear();
return result;
```

Only the first unreachable statement of each block is reported. Stray `;` are left to
`empty-statement`."#,
    ..RuleMeta::new("java/unreachable-code", "java", "Unreachable code")
};

/// Remembers the last block reported, so each block yields at most one finding.
#[derive(Default)]
pub struct UnreachableCode {
    last_block: Option<usize>,
}

impl<'t> JavaRule<'t> for UnreachableCode {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "return_statement",
            "throw_statement",
            "break_statement",
            "continue_statement",
            "yield_statement",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(block) = ctx.parent(node) else {
            return;
        };
        if !matches!(
            kind(block),
            "block" | "constructor_body" | "switch_block_statement_group"
        ) || block.has_error()
            || self.last_block == Some(block.id())
        {
            return;
        }
        // The next statement of the same block (comments skipped).
        let mut next = ctx.next_named_sibling(node);
        while let Some(n) = next {
            if !syntax::is_comment(n) {
                break;
            }
            next = ctx.next_named_sibling(n);
        }
        let Some(dead) = next else { return };
        if kind(dead) == "switch_label" {
            return;
        }
        self.last_block = Some(block.id());
        let kw = syntax::children(node)
            .next()
            .map(|t| ctx.text(t))
            .unwrap_or("return");
        ctx.emit(
            dead,
            format!("Remove this unreachable code or move it before the `{kw}`"),
        )
        .secondary(node, "control never gets past this statement");
    }
}
