//! `java/misleading-if-layout` — an `if` written on the closing-brace line of the previous
//! `if` (it reads like `else if`), or an `else` aligned with an outer `if` it does not belong to.

use super::util::keyword;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["confusing", "braces"],
    remediation_minutes: 2,
    description: r#"Detects two layouts that make an `if` read differently from how it runs:

- an `if` that starts on the line of the closing brace of the previous `if` statement
  (`} if (…) {`): it reads like `else if`, but it is a separate statement that is evaluated
  even when the previous branch ran;
- a dangling `else` placed under an outer braceless `if`: `else` always belongs to the nearest
  `if`, so the branch runs when the *inner* condition is false, not the outer one.

**Why it matters.** Both are typical slips: a forgotten `else` turns a chain of exclusive
cases into independent tests (the first case falls into the checks of the next ones), and an
`else` indented for the wrong `if` runs in exactly the situations the reader thinks it
doesn't.

**How to fix.** Add the missing `else`, or put the second `if` on its own line if the tests
really are independent. For a dangling `else`, add braces around the inner `if`.

```java
// Noncompliant
if (value instanceof Map) {
    return fromMap(value);
} if (value instanceof List) {      // meant: else if
    return fromList(value);
}

if (user != null)
    if (user.isAdmin())
        grant();
else                                // belongs to `if (user.isAdmin())`
    login();

// Compliant
if (value instanceof Map) {
    return fromMap(value);
} else if (value instanceof List) {
    return fromList(value);
}

if (user != null) {
    if (user.isAdmin())
        grant();
} else {
    login();
}
```

Only an `if` directly following another `if` statement whose last branch is a block is
checked for the first shape; for the second, the `else` must start its line in the column of
the outer `if`."#,
    ..RuleMeta::new(
        "java/misleading-if-layout",
        "java",
        "if laid out as if it were an else branch",
    )
};

pub struct MisleadingIfLayout;

impl<'t> JavaRule<'t> for MisleadingIfLayout {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["if_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        // Cheap text and shape tests before any tree navigation.
        let before = ctx.source().get(..node.start_byte()).unwrap_or("");
        let after_brace = before.trim_end_matches([' ', '\t']).ends_with('}');
        let dangling = node.field(Field::Alternative).is_none()
            && node
                .field(Field::Consequence)
                .is_some_and(|c| kind(c) == "if_statement");
        if !after_brace && !dangling {
            return;
        }
        let Some(if_kw) = keyword(node, "if") else {
            return;
        };
        if after_brace {
            check_same_line(node, if_kw, ctx);
        }
        if dangling {
            check_dangling_else(node, if_kw, ctx);
        }
    }
}

/// The last branch of an `if` chain (the final `else` body, or the last consequence).
fn last_branch(stmt: Node<'_>) -> Option<Node<'_>> {
    let mut cur = stmt;
    for _ in 0..4096 {
        match cur.field(Field::Alternative) {
            Some(alt) if kind(alt) == "if_statement" => cur = alt,
            Some(alt) => return Some(alt),
            None => return cur.field(Field::Consequence),
        }
    }
    None
}

fn check_same_line<'t>(node: Node<'t>, if_kw: Node<'t>, ctx: &Ctx<'t>) {
    let Some(parent) = ctx.parent(node) else {
        return;
    };
    if !matches!(
        kind(parent),
        "block" | "constructor_body" | "switch_block_statement_group"
    ) {
        return;
    }
    let Some(prev) = ctx.prev_named_sibling(node) else {
        return;
    };
    if kind(prev) != "if_statement" || prev.end_position().row != if_kw.start_position().row {
        return;
    }
    if last_branch(prev).is_none_or(|b| kind(b) != "block") {
        return;
    }
    ctx.emit(
        if_kw,
        "Add `else` or start this `if` on its own line; it reads like `else if` but runs independently",
    );
}

/// Column (in bytes) of `node` on its line, and whether only whitespace precedes it.
fn column(node: Node<'_>, src: &str) -> (usize, bool) {
    let start = node.start_byte();
    let line_start = src[..start].rfind('\n').map_or(0, |p| p + 1);
    let before = &src[line_start..start];
    (
        start - line_start,
        before.chars().all(|c| c == ' ' || c == '\t'),
    )
}

fn check_dangling_else<'t>(node: Node<'t>, if_kw: Node<'t>, ctx: &Ctx<'t>) {
    if node.field(Field::Alternative).is_some() {
        return;
    }
    let Some(inner) = node.field(Field::Consequence) else {
        return;
    };
    if kind(inner) != "if_statement" || inner.field(Field::Alternative).is_none() {
        return;
    }
    let (Some(inner_if), Some(else_kw)) = (keyword(inner, "if"), keyword(inner, "else")) else {
        return;
    };
    let src = ctx.source();
    let (outer_col, outer_first) = column(if_kw, src);
    let (inner_col, _) = column(inner_if, src);
    let (else_col, else_first) = column(else_kw, src);
    if !outer_first || !else_first || else_col != outer_col || inner_col == outer_col {
        return;
    }
    // Tabs and spaces must not be equated: compare the literal indentation.
    let indent = |n: Node<'_>| {
        let start = n.start_byte();
        let line_start = src[..start].rfind('\n').map_or(0, |p| p + 1);
        &src[line_start..start]
    };
    if indent(if_kw) != indent(else_kw) {
        return;
    }
    ctx.emit(
        else_kw,
        "Add braces around the inner `if`; this `else` belongs to it, not to the `if` it is aligned with",
    );
}
