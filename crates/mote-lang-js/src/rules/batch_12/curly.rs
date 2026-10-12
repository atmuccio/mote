//! `js/curly` (off by default; part of the `conventions` set)

use crate::prelude::*;

pub struct Curly;

pub static CURLY: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["conventions", "braces"],
    remediation_minutes: 1,
    params: &[ParamSpec {
        name: "single_line_if",
        default: ParamDefault::Bool(true),
        description: "Accept an `if` without `else` written entirely on one line without braces (`if (done) return;`); when false every body needs braces.",
    }],
    description: r#"Flags `if`, `else`, `for`, `while` and `do` bodies that are a single statement without braces.

**Why:** a braceless body ends after one statement, whatever the indentation says. A second
line added later at the same indentation looks like part of the body but always runs, and a
nested `if` without braces can take the wrong `else`. Always writing the braces removes that
trap and keeps diffs small when a body grows.

**Fix:** wrap the body in `{ … }`.

```js
if (user)
  greet(user);              // flagged
for (const item of items) process(item);   // flagged: loops always take braces

if (user) {
  greet(user);              // compliant
}
if (!user) return;          // compliant: a one-line if without else
```

By default an `if` that has no `else` and fits on one line (condition and statement) may skip
the braces, as the common style guides allow; the `single_line_if` parameter turns that off. An
`else if` chain is not reported for the `else` (the nested `if` is checked itself), but an `if`
in such a chain gets no one-line exemption.

Off by default: part of the opt-in `conventions` set.
"#,
    ..RuleMeta::new("js/curly", "js", "Control statement bodies should be wrapped in braces")
};

fn is_block(stmt: &Statement<'_>) -> bool {
    matches!(stmt, Statement::BlockStatement(_))
}

/// The `else` keyword between the end of `consequent` and the start of `alternate`, skipping
/// comments.
fn else_keyword(ctx: &Ctx<'_>, from: u32, to: u32) -> Option<Span> {
    let gap = ctx.text(Span::new(from, to));
    let comments = ctx.comments();
    let mut search = 0;
    while let Some(found) = gap.get(search..).and_then(|rest| rest.find("else")) {
        let start = from + u32::try_from(search + found).ok()?;
        let in_comment = comments
            .iter()
            .any(|c| c.span.start <= start && start < c.span.end);
        if !in_comment {
            return Some(Span::new(start, start + 4));
        }
        search += found + 4;
    }
    None
}

impl JsRule for Curly {
    fn meta(&self) -> &'static RuleMeta {
        &CURLY
    }

    fn types(&self) -> &'static [AstType] {
        &[
            AstType::IfStatement,
            AstType::ForStatement,
            AstType::ForInStatement,
            AstType::ForOfStatement,
            AstType::WhileStatement,
            AstType::DoWhileStatement,
        ]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let (start, body, keyword) = match node.kind() {
            AstKind::IfStatement(stmt) => {
                check_if(ctx, node, stmt);
                return;
            }
            AstKind::ForStatement(s) => (s.span.start, &s.body, "for"),
            AstKind::ForInStatement(s) => (s.span.start, &s.body, "for"),
            AstKind::ForOfStatement(s) => (s.span.start, &s.body, "for"),
            AstKind::WhileStatement(s) => (s.span.start, &s.body, "while"),
            AstKind::DoWhileStatement(s) => (s.span.start, &s.body, "do"),
            _ => return,
        };
        if is_block(body) {
            return;
        }
        ctx.emit(
            Span::new(start, start + keyword.len() as u32),
            format!("Wrap the body of this '{keyword}' in braces"),
        );
    }
}

fn check_if<'a>(ctx: &mut Ctx<'a>, node: &AstNode<'a>, stmt: &'a IfStatement<'a>) {
    if !is_block(&stmt.consequent) {
        let in_else_chain = matches!(
            ctx.parent_kind(node),
            Some(AstKind::IfStatement(parent))
                if parent.alternate.as_ref().is_some_and(|alt| alt.span() == stmt.span)
        );
        let one_line = stmt.alternate.is_none()
            && !in_else_chain
            && ctx.line_of(stmt.span.start) == ctx.line_of(stmt.consequent.span().end)
            && ctx.param_bool("single_line_if");
        if !one_line {
            ctx.emit(
                Span::new(stmt.span.start, stmt.span.start + 2),
                "Wrap the body of this 'if' in braces",
            );
        }
    }
    if let Some(alternate) = &stmt.alternate {
        if is_block(alternate) || matches!(alternate, Statement::IfStatement(_)) {
            return;
        }
        let span = else_keyword(ctx, stmt.consequent.span().end, alternate.span().start)
            .unwrap_or_else(|| alternate.span());
        ctx.emit(span, "Wrap the 'else' branch in braces");
    }
}
