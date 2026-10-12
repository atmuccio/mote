//! `js/misleading-if-layout`

use crate::prelude::*;

pub struct MisleadingIfLayout;

pub static MISLEADING_IF_LAYOUT: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["confusing", "braces"],
    remediation_minutes: 2,
    description: r#"Flags two layouts that make an `if` read differently from how it runs:

- an `if` written on the line of the closing brace of the `if` statement before it
  (`} if (…) {`): it reads like `else if`, but it is a separate statement, tested even when the
  branch above ran;
- an `else` lined up under an outer brace-less `if` while it belongs to an inner one: `else`
  always pairs with the nearest `if`, so the branch runs when the *inner* condition is false,
  not the outer one.

**Why:** both are typical slips. A missing `else` turns a chain of exclusive cases into
independent tests, so a value handled by the first branch falls into the checks of the next ones
too; an `else` indented for the wrong `if` runs in exactly the cases the reader thinks it does
not.

**Fix:** add the missing `else`, or start the second `if` on its own line if the tests really
are independent. For the `else`, put braces around the inner `if`.

```js
if (value instanceof Map) {
  handleMap(value);
} if (Array.isArray(value)) {      // flagged: meant `else if`?
  handleList(value);
}

if (user)
  if (user.admin)
    grant();
else                               // flagged: belongs to `if (user.admin)`
  login();

if (value instanceof Map) {
  handleMap(value);
} else if (Array.isArray(value)) { // compliant
  handleList(value);
}
```

For the first layout, only an `if` whose line starts with the closing brace of an `if` statement
whose last branch is a block is checked (one-line code such as minified bundles is not), and only
when some branch of that statement can run to its end: when every branch returns, throws, breaks
or continues, the second `if` behaves like `else if` anyway. A comment between the brace and the
`if` counts as a deliberate separation. For the second, the
`else` must start its line with exactly the indentation of the outer `if`, which must start its
line too, and the inner `if` must sit at another column.
"#,
    ..RuleMeta::new(
        "js/misleading-if-layout",
        "js",
        "An if should not be laid out as if it were an else branch",
    )
};

impl JsRule for MisleadingIfLayout {
    fn meta(&self) -> &'static RuleMeta {
        &MISLEADING_IF_LAYOUT
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::IfStatement]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::IfStatement(stmt) = node.kind() else {
            return;
        };
        // Cheap text and shape tests before any tree navigation: a `}` right before the `if`, or
        // an inner `if` with an `else`.
        let start = stmt.span.start;
        let after_brace = ctx
            .source()
            .get(..start as usize)
            .is_some_and(|before| before.trim_end_matches([' ', '\t']).ends_with('}'));
        if after_brace {
            let prefix = line_prefix(ctx, start);
            if prefix.trim() == "}" {
                check_same_line(ctx, node, stmt, prefix);
            }
        }
        if stmt.alternate.is_none() {
            if let Statement::IfStatement(inner) = &stmt.consequent {
                if inner.alternate.is_some() {
                    check_dangling_else(ctx, stmt, inner, line_prefix(ctx, start));
                }
            }
        }
    }
}

/// `} if (…)`: the `if` shares the line of the previous `if` statement's closing brace.
fn check_same_line<'a>(
    ctx: &mut Ctx<'a>,
    node: &AstNode<'a>,
    stmt: &IfStatement<'a>,
    prefix: &str,
) {
    let statements: &'a [Statement<'a>] = match ctx.parent_kind(node) {
        Some(AstKind::BlockStatement(b)) => &b.body,
        Some(AstKind::FunctionBody(f)) => &f.statements,
        Some(AstKind::Program(p)) => &p.body,
        Some(AstKind::SwitchCase(c)) => &c.consequent,
        Some(AstKind::StaticBlock(s)) => &s.body,
        Some(AstKind::TSModuleBlock(m)) => &m.body,
        _ => return,
    };
    let start = stmt.span.start;
    let index = statements.partition_point(|s| s.span().start < start);
    let Some(Statement::IfStatement(previous)) =
        index.checked_sub(1).and_then(|i| statements.get(i))
    else {
        return;
    };
    // The brace on this line must be the end of that `if` statement.
    let Some(brace) = prefix.find('}') else {
        return;
    };
    let line_start = start - prefix.len() as u32;
    if previous.span.end != line_start + brace as u32 + 1
        || !last_branch_is_block(previous)
        || !some_branch_completes(previous)
    {
        return;
    }
    ctx.emit(
        Span::new(start, start + 2),
        "Add 'else' or start this 'if' on its own line: it reads like 'else if' but is tested even when the branch above ran",
    )
    .secondary(
        Span::new(previous.span.start, previous.span.start + 2),
        "The previous 'if' statement",
    );
}

/// The last branch of an `if … else if … else` chain is a block.
fn last_branch_is_block(stmt: &IfStatement<'_>) -> bool {
    let mut current = stmt;
    // Guard against pathological chains.
    for _ in 0..4096 {
        match &current.alternate {
            Some(Statement::IfStatement(next)) => current = next,
            Some(other) => return matches!(other, Statement::BlockStatement(_)),
            None => return matches!(current.consequent, Statement::BlockStatement(_)),
        }
    }
    false
}

/// Can some branch of the `if … else if … else` chain run to its end? When every branch returns,
/// throws, breaks or continues, the next `if` behaves exactly like an `else if`.
fn some_branch_completes(stmt: &IfStatement<'_>) -> bool {
    let mut current = stmt;
    // Guard against pathological chains.
    for _ in 0..4096 {
        if !ends_in_jump(&current.consequent) {
            return true;
        }
        match &current.alternate {
            Some(Statement::IfStatement(next)) => current = next,
            Some(other) => return !ends_in_jump(other),
            None => return false,
        }
    }
    true
}

/// The branch ends with `return`, `throw`, `break` or `continue`.
fn ends_in_jump(branch: &Statement<'_>) -> bool {
    let last = match branch {
        Statement::BlockStatement(block) => block.body.last(),
        other => Some(other),
    };
    matches!(
        last,
        Some(
            Statement::ReturnStatement(_)
                | Statement::ThrowStatement(_)
                | Statement::BreakStatement(_)
                | Statement::ContinueStatement(_)
        )
    )
}

/// `if (a)` / `  if (b) x();` / `else y();`: the `else` is aligned with the outer `if` but
/// belongs to the inner one.
fn check_dangling_else<'a>(
    ctx: &mut Ctx<'a>,
    outer: &IfStatement<'a>,
    inner: &IfStatement<'a>,
    outer_prefix: &str,
) {
    let Some(alternate) = &inner.alternate else {
        return;
    };
    if !is_blank(outer_prefix) {
        return;
    }
    let gap = Span::new(inner.consequent.span().end, alternate.span().start);
    let Some(at) = ctx.text(gap).rfind("else") else {
        return;
    };
    let else_at = gap.start + at as u32;
    let else_prefix = line_prefix(ctx, else_at);
    // Tabs and spaces are not equated: the indentation must be literally the same.
    if !is_blank(else_prefix) || else_prefix != outer_prefix {
        return;
    }
    let inner_prefix = line_prefix(ctx, inner.span.start);
    if ctx.line_of(inner.span.start) != ctx.line_of(outer.span.start)
        && inner_prefix == outer_prefix
    {
        return;
    }
    ctx.emit(
        Span::new(else_at, else_at + 4),
        "Add braces around the inner 'if': this 'else' belongs to it, not to the 'if' it is aligned with",
    )
    .secondary(
        Span::new(inner.span.start, inner.span.start + 2),
        "The 'if' this 'else' belongs to",
    );
}

fn is_blank(text: &str) -> bool {
    text.chars().all(|c| c == ' ' || c == '\t')
}

/// The text of the line that contains `offset`, from the start of the line up to `offset`.
fn line_prefix<'a>(ctx: &Ctx<'a>, offset: u32) -> &'a str {
    let Some(before) = ctx.source().get(..offset as usize) else {
        return "";
    };
    let start = before.rfind('\n').map_or(0, |i| i + 1);
    before.get(start..).unwrap_or("")
}
