//! `js/no-lonely-if`

use crate::prelude::*;

pub struct NoLonelyIf;

pub static NO_LONELY_IF: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["readability"],
    remediation_minutes: 2,
    description: r#"Flags an `else` block whose only statement is an `if` statement.

**Why:** `else { if (…) … }` adds a level of braces and indentation for nothing; `else if` says
the same thing and keeps the chain flat and easy to scan.

**Fix:** merge the block into an `else if`.

```js
if (a) {
  run();
} else {
  if (b) {          // flagged
    stop();
  }
}

if (a) {            // compliant
  run();
} else if (b) {
  stop();
}
```
"#,
    ..RuleMeta::new(
        "js/no-lonely-if",
        "js",
        "An 'if' should not be the only statement of an 'else' block",
    )
};

impl JsRule for NoLonelyIf {
    fn meta(&self) -> &'static RuleMeta {
        &NO_LONELY_IF
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::IfStatement]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::IfStatement(stmt) = node.kind() else {
            return;
        };
        let Some(block_node) = ctx.parent(node) else {
            return;
        };
        let AstKind::BlockStatement(block) = block_node.kind() else {
            return;
        };
        if block.body.len() != 1 {
            return;
        }
        let Some(AstKind::IfStatement(outer)) = ctx.parent_kind(block_node) else {
            return;
        };
        if outer.alternate.as_ref().map(GetSpan::span) != Some(block.span) {
            return;
        }
        ctx.emit(
            Span::new(stmt.span.start, stmt.span.start + 2),
            "Replace this 'else { if … }' with 'else if'",
        );
    }
}
