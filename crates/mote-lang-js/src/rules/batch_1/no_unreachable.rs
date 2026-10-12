//! `js/no-unreachable`

use super::util;
use crate::prelude::*;

pub struct NoUnreachable;

pub static NO_UNREACHABLE: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["suspicious", "unused"],
    remediation_minutes: 5,
    description: r#"Flags statements that can never run because they follow a `return`, `throw`, `break`, or
`continue` in the same block (directly, or after an `if`/`else` or `try` whose every path leaves).

**Why:** unreachable code is dead weight at best. More often it is a bug: a debugging `return`
left in place, or a statement accidentally placed after the exit it was meant to precede, so the
code the author expected to run never does.

**Fix:** remove the dead statements, or move them before the exit.

```js
function save(record) {
  return store.put(record);
  audit(record);            // flagged: never runs
}

function load() {
  return read();
  function read() {}        // compliant: function declarations are hoisted
}
```

Hoisted declarations (functions, `var` without initializer), TypeScript type declarations, and a
redundant plain `break;` (as in `return x; break;` at the end of a `switch` case) are not
reported. Loops, `switch` statements, and labeled blocks are conservatively assumed to complete
normally.
"#,
    ..RuleMeta::new(
        "js/no-unreachable",
        "js",
        "Unreachable code should be removed",
    )
};

impl JsRule for NoUnreachable {
    fn meta(&self) -> &'static RuleMeta {
        &NO_UNREACHABLE
    }

    fn types(&self) -> &'static [AstType] {
        &[
            AstType::Program,
            AstType::BlockStatement,
            AstType::FunctionBody,
            AstType::SwitchCase,
            AstType::StaticBlock,
            AstType::TSModuleBlock,
        ]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let statements: &[Statement<'a>] = match node.kind() {
            AstKind::Program(p) => &p.body,
            AstKind::BlockStatement(b) => &b.body,
            AstKind::FunctionBody(b) => &b.statements,
            AstKind::SwitchCase(c) => &c.consequent,
            AstKind::StaticBlock(b) => &b.body,
            AstKind::TSModuleBlock(b) => &b.body,
            _ => return,
        };
        if statements.len() < 2 {
            return;
        }
        let Some(exit) = statements.iter().position(util::always_exits) else {
            return;
        };
        let Some(dead) = statements[exit + 1..].iter().find(|s| !ignorable(s)) else {
            return;
        };
        let span = dead.span();
        let first_line_end = ctx
            .text(span)
            .find('\n')
            .map_or(span.end, |i| span.start + i as u32);
        let span = Span::new(span.start, first_line_end.max(span.start));
        ctx.emit(
            span,
            "Remove this unreachable code or move it before the exit",
        );
    }
}

/// Statements that are harmless after an exit: hoisted or type-only declarations, empty
/// statements, and a redundant plain `break`.
fn ignorable(stmt: &Statement<'_>) -> bool {
    match stmt {
        Statement::FunctionDeclaration(_)
        | Statement::EmptyStatement(_)
        | Statement::TSTypeAliasDeclaration(_)
        | Statement::TSInterfaceDeclaration(_)
        | Statement::TSModuleDeclaration(_)
        | Statement::TSGlobalDeclaration(_)
        | Statement::ImportDeclaration(_)
        | Statement::ExportAllDeclaration(_) => true,
        Statement::BreakStatement(b) => b.label.is_none(),
        Statement::VariableDeclaration(v) => {
            v.declare
                || (v.kind == VariableDeclarationKind::Var
                    && v.declarations.iter().all(|d| d.init.is_none()))
        }
        Statement::ClassDeclaration(c) => c.declare,
        Statement::TSEnumDeclaration(e) => e.declare,
        Statement::ExportNamedDeclaration(e) => e.declaration.as_ref().is_none_or(|d| {
            matches!(
                d,
                Declaration::FunctionDeclaration(_)
                    | Declaration::TSTypeAliasDeclaration(_)
                    | Declaration::TSInterfaceDeclaration(_)
            )
        }),
        _ => false,
    }
}
