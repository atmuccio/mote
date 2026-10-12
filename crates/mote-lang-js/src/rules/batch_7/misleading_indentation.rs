//! `js/misleading-indentation`

use super::util::{indentation_at, starts_line};
use crate::prelude::*;
use crate::rules::batch_6::util::first_line;

pub struct MisleadingIndentation;

pub static MISLEADING_INDENTATION: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["suspicious", "confusing"],
    remediation_minutes: 2,
    description: r#"Flags a statement that is indented like the body of the brace-less `if`, `else`, `for` or `while`
above it, although it is not part of that body and always runs.

**Why:** without braces, only the first statement after `if (…)` is conditional. A second
statement written at the same indentation reads as if it were guarded too — the classic way a
check or a cleanup ends up running unconditionally (or a new line is added "inside" an `if` that
does not contain it).

**Fix:** add braces if both statements belong to the body, or outdent the statement to the level
of the `if` if it is meant to run every time.

```js
if (!user)
  log('no user');
  return;             // flagged: runs every time

if (!user) {
  log('no user');
  return;             // compliant
}
```

Only a body written on its own line is checked, and the next statement must start its own line
with exactly the same indentation as that body, deeper than the header. Bodies in braces and
statements on the same line are left alone.
"#,
    ..RuleMeta::new(
        "js/misleading-indentation",
        "js",
        "Statements should not be indented as if a brace-less body guarded them",
    )
};

impl JsRule for MisleadingIndentation {
    fn meta(&self) -> &'static RuleMeta {
        &MISLEADING_INDENTATION
    }

    fn types(&self) -> &'static [AstType] {
        &[
            AstType::BlockStatement,
            AstType::FunctionBody,
            AstType::Program,
            AstType::SwitchCase,
            AstType::StaticBlock,
            AstType::TSModuleBlock,
        ]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let statements: &'a [Statement<'a>] = match node.kind() {
            AstKind::BlockStatement(b) => &b.body,
            AstKind::FunctionBody(f) => &f.statements,
            AstKind::Program(p) => &p.body,
            AstKind::SwitchCase(c) => &c.consequent,
            AstKind::StaticBlock(s) => &s.body,
            AstKind::TSModuleBlock(m) => &m.body,
            _ => return,
        };
        for pair in statements.windows(2) {
            let [current, next] = pair else {
                continue;
            };
            check(ctx, current, next);
        }
    }
}

fn check<'a>(ctx: &mut Ctx<'a>, current: &'a Statement<'a>, next: &'a Statement<'a>) {
    // Cheapest test first: only compound statements without braces matter.
    if !matches!(
        current,
        Statement::IfStatement(_)
            | Statement::ForStatement(_)
            | Statement::ForInStatement(_)
            | Statement::ForOfStatement(_)
            | Statement::WhileStatement(_)
            | Statement::LabeledStatement(_)
    ) {
        return;
    }
    let next_start = next.span().start;
    if matches!(next, Statement::EmptyStatement(_)) || !starts_line(ctx, next_start) {
        return;
    }
    let next_indent = indentation_at(ctx, next_start);
    let outer_indent = indentation_at(ctx, current.span().start);
    if next_indent.len() <= outer_indent.len() {
        return;
    }
    let mut stmt = current;
    // Guard against pathological nesting.
    for _ in 0..32 {
        let Some((body, header_at, keyword)) = guarded_body(ctx, stmt) else {
            return;
        };
        if matches!(
            body,
            Statement::BlockStatement(_) | Statement::EmptyStatement(_)
        ) {
            return;
        }
        let body_start = body.span().start;
        if ctx.line_of(body_start) != ctx.line_of(header_at) && starts_line(ctx, body_start) {
            let body_indent = indentation_at(ctx, body_start);
            let header_indent = indentation_at(ctx, header_at);
            if body_indent == next_indent
                && body_indent.len() > header_indent.len()
                && body_indent.starts_with(header_indent)
            {
                let header_span = Span::new(header_at, header_at + keyword.len() as u32);
                ctx.emit(
                    first_line(ctx, next.span()),
                    format!(
                        "This statement is indented as if the brace-less '{keyword}' above guarded it, but it always runs: add braces or fix the indentation"
                    ),
                )
                .secondary(header_span, format!("The '{keyword}' only guards the next statement"));
                return;
            }
        }
        stmt = body;
    }
}

/// The body that runs under `stmt`'s condition when it is a brace-less compound statement, with
/// the offset and text of the keyword that guards it.
fn guarded_body<'a>(
    ctx: &Ctx<'a>,
    stmt: &'a Statement<'a>,
) -> Option<(&'a Statement<'a>, u32, &'static str)> {
    match stmt {
        Statement::IfStatement(s) => match &s.alternate {
            None => Some((&s.consequent, s.span.start, "if")),
            Some(alternate) => {
                // Only the `else` branch can be mistaken for the body of what follows.
                let gap = Span::new(s.consequent.span().end, alternate.span().start);
                let at = ctx.text(gap).rfind("else")?;
                Some((alternate, gap.start + at as u32, "else"))
            }
        },
        Statement::ForStatement(s) => Some((&s.body, s.span.start, "for")),
        Statement::ForInStatement(s) => Some((&s.body, s.span.start, "for")),
        Statement::ForOfStatement(s) => Some((&s.body, s.span.start, "for")),
        Statement::WhileStatement(s) => Some((&s.body, s.span.start, "while")),
        Statement::LabeledStatement(s) => guarded_body(ctx, &s.body),
        _ => None,
    }
}
