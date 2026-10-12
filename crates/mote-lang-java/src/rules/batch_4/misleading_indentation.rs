//! `java/misleading-indentation` — a statement indented as if a braceless body contained it.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["confusing", "braces"],
    remediation_minutes: 2,
    description: r#"Detects a statement that follows a braceless `if`, `else`, `for`, or `while`
body and is indented exactly like that body, so it looks guarded but always runs.

**Why it matters.** Without braces only the first statement belongs to the body. A second
line added at the same indentation — often during a quick fix — reads as part of the
branch or loop, but executes unconditionally (or once, after the loop). This is the shape
of several well-known security bugs.

**How to fix.** Add braces around the body, or fix the indentation of the following
statement if it really is meant to run unconditionally.

```java
// Noncompliant
if (user.isAdmin())
    grantAccess();
    auditLog.record(user);

// Compliant
if (user.isAdmin()) {
    grantAccess();
    auditLog.record(user);
}
```

Only bodies that start on their own line are considered, and indentation is compared as
the literal leading whitespace of each line (tabs and spaces are never equated)."#,
    ..RuleMeta::new(
        "java/misleading-indentation",
        "java",
        "Misleading indentation after a braceless body",
    )
};

pub struct MisleadingIndentation;

/// The last body of a statement and the keyword token (`if`, `else`, `for`, `while`) owning it.
fn tail_body<'t>(stmt: Node<'t>) -> Option<(Node<'t>, Node<'t>)> {
    match kind(stmt) {
        "if_statement" => {
            let mut cur = stmt;
            for _ in 0..1024 {
                match util::fld(cur, F::Alternative) {
                    Some(alt) if kind(alt) == "if_statement" => cur = alt,
                    Some(alt) => {
                        let else_kw = syntax::children(cur).find(|c| kind(*c) == "else")?;
                        return Some((alt, else_kw));
                    }
                    None => {
                        let if_kw = syntax::children(cur).next()?;
                        return Some((util::fld(cur, F::Consequence)?, if_kw));
                    }
                }
            }
            None
        }
        "for_statement" | "enhanced_for_statement" | "while_statement" => {
            Some((util::fld(stmt, F::Body)?, syntax::children(stmt).next()?))
        }
        _ => None,
    }
}

impl MisleadingIndentation {
    /// `stmt` (an `if`/`for`/`while`) directly followed by `next` in the same statement list.
    fn check_pair<'t>(&self, ctx: &Ctx<'t>, stmt: Node<'t>, next: Node<'t>) {
        let Some((body, keyword)) = tail_body(stmt) else {
            return;
        };
        if kind(body) == "block" {
            return;
        }
        let src = ctx.source();
        let body_row = body.start_position().row;
        let next_row = next.start_position().row;
        if body_row <= keyword.start_position().row || next_row <= body.end_position().row {
            return;
        }
        let (kw_indent, _) = util::line_indent(src, keyword.start_byte());
        let (body_indent, body_first) = util::line_indent(src, body.start_byte());
        let (next_indent, next_first) = util::line_indent(src, next.start_byte());
        if !body_first || !next_first || body_indent != next_indent {
            return;
        }
        // The body must really be indented deeper than its keyword, with the same characters.
        if body_indent.len() <= kw_indent.len() || !body_indent.starts_with(kw_indent) {
            return;
        }
        let what = match kind(stmt) {
            "if_statement" => "the `if`/`else` above",
            _ => "the loop above",
        };
        // Report the first line of the misleading statement only.
        let start = next.start_byte();
        let end = src
            .get(start..next.end_byte())
            .and_then(|t| t.find('\n'))
            .map_or(next.end_byte(), |p| start + p);
        ctx.emit_range(
            Span::new(start, end),
            format!("Add braces or fix the indentation: this statement is indented as if it belonged to {what}, but it always runs"),
        );
    }
}

impl<'t> JavaRule<'t> for MisleadingIndentation {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "if_statement",
            "for_statement",
            "enhanced_for_statement",
            "while_statement",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        // Only statements of a statement list have a following statement; `else if` chains
        // and nested bodies are handled from their outermost statement. (Tested first: finding
        // the tail of an `else if` chain from every `if` of it was quadratic in its length.)
        let Some(parent) = ctx.parent(node) else {
            return;
        };
        if !matches!(
            kind(parent),
            "block" | "constructor_body" | "switch_block_statement_group"
        ) {
            return;
        }
        // Most bodies have braces.
        let Some((body, _)) = tail_body(node) else {
            return;
        };
        if kind(body) == "block" {
            return;
        }
        let mut siblings = named_children(parent).take(100_000);
        if siblings.by_ref().any(|s| s.id() == node.id()) {
            if let Some(next) = siblings.next() {
                self.check_pair(ctx, node, next);
            }
        }
    }
}
