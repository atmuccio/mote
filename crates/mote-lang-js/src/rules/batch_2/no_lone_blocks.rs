//! `js/no-lone-blocks`

use super::util;
use crate::prelude::*;

pub struct NoLoneBlocks;

pub static NO_LONE_BLOCKS: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["redundant"],
    remediation_minutes: 1,
    description: r#"Flags stand-alone `{ … }` blocks nested directly in another block, a function
body, or the top level, when they do not declare anything scoped to the block.

**Why:** since `var` and plain statements ignore block boundaries, such a block does not create
any scope; it only adds indentation and suggests a structure (a missing `if`, a lost loop) that is
not there.

**Fix:** remove the braces, or turn the block into a function if it groups a distinct step.
Blocks that declare `let`, `const`, `class`, functions, or TypeScript types are compliant (the
block limits their scope), as are labeled blocks, the single block of a `case` clause, and
blocks directly preceded by a comment (a named section of a long function). Empty
blocks are left to `js/no-empty`.

```js
function run() {
  prepare();
  {                  // flagged
    execute();
  }
  {                  // compliant: limits the scope of `tmp`
    const tmp = load();
    use(tmp);
  }
}
```
"#,
    ..RuleMeta::new(
        "js/no-lone-blocks",
        "js",
        "Nested blocks should not be redundant",
    )
};

fn is_scoped_declaration(stmt: &Statement<'_>) -> bool {
    match stmt {
        Statement::VariableDeclaration(d) => d.kind != VariableDeclarationKind::Var,
        Statement::FunctionDeclaration(_)
        | Statement::ClassDeclaration(_)
        | Statement::TSTypeAliasDeclaration(_)
        | Statement::TSInterfaceDeclaration(_)
        | Statement::TSEnumDeclaration(_)
        | Statement::TSModuleDeclaration(_)
        | Statement::TSGlobalDeclaration(_)
        | Statement::TSImportEqualsDeclaration(_) => true,
        _ => false,
    }
}

impl JsRule for NoLoneBlocks {
    fn meta(&self) -> &'static RuleMeta {
        &NO_LONE_BLOCKS
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::BlockStatement]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::BlockStatement(block) = node.kind() else {
            return;
        };
        if block.body.is_empty() || block.body.iter().any(is_scoped_declaration) {
            return;
        }
        match ctx.parent_kind(node) {
            Some(
                AstKind::BlockStatement(_)
                | AstKind::FunctionBody(_)
                | AstKind::Program(_)
                | AstKind::StaticBlock(_)
                | AstKind::TSModuleBlock(_),
            ) => {}
            // `case x: { … }` is a common way to delimit a clause.
            Some(AstKind::SwitchCase(case)) => {
                if case.consequent.len() == 1 {
                    return;
                }
            }
            _ => return,
        }
        // `// Section name` + `{ … }`: a deliberately delimited section of a long function.
        if util::has_comment_before(ctx, block.span.start) {
            return;
        }
        ctx.emit(
            Span::new(block.span.start, block.span.start + 1),
            "Remove this redundant block and keep its statements",
        );
    }
}
