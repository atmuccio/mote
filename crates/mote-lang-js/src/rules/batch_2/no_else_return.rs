//! `js/no-else-return`

use super::util;
use crate::prelude::*;

pub struct NoElseReturn;

pub static NO_ELSE_RETURN: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["readability"],
    remediation_minutes: 2,
    description: r#"Flags an `else` block that follows an `if` whose branch always ends with `return`.

**Why:** when the `if` branch returns, the code after it only runs in the other case anyway. The
`else` adds a level of nesting and makes the function's main path harder to follow.

**Fix:** remove the `else` and move its statements after the `if`. In an `if … else if … else`
chain, the final `else` is reported only when *every* branch before it returns, so removing it
cannot change behavior. `else if` itself is never reported.

```js
function sign(n) {
  if (n < 0) {
    return -1;
  } else {            // flagged
    return 1;
  }
}

function sign(n) {    // compliant
  if (n < 0) {
    return -1;
  }
  return 1;
}
```
"#,
    // Stylistic and noisy on real code: opt-in (`extends: all` or enable explicitly).
    default_enabled: false,
    ..RuleMeta::new(
        "js/no-else-return",
        "js",
        "'else' should not follow an 'if' block that returns",
    )
};

/// Does every path through `stmt` end in a `return`?
fn always_returns(stmt: &Statement<'_>) -> bool {
    match stmt {
        Statement::ReturnStatement(_) => true,
        Statement::BlockStatement(b) => b.body.last().is_some_and(always_returns),
        Statement::IfStatement(i) => {
            always_returns(&i.consequent) && i.alternate.as_ref().is_some_and(always_returns)
        }
        _ => false,
    }
}

impl JsRule for NoElseReturn {
    fn meta(&self) -> &'static RuleMeta {
        &NO_ELSE_RETURN
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::IfStatement]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::IfStatement(stmt) = node.kind() else {
            return;
        };
        if stmt.alternate.is_none() {
            return;
        }
        // Start from the head of an if/else-if chain that sits in a statement list, where the
        // `else` body can be moved after the `if`.
        match ctx.parent_kind(node) {
            Some(
                AstKind::Program(_)
                | AstKind::BlockStatement(_)
                | AstKind::FunctionBody(_)
                | AstKind::StaticBlock(_)
                | AstKind::SwitchCase(_)
                | AstKind::TSModuleBlock(_),
            ) => {}
            _ => return,
        }
        let mut current: &IfStatement<'a> = stmt;
        let last_else = loop {
            if !always_returns(&current.consequent) {
                return;
            }
            match &current.alternate {
                None => return,
                Some(Statement::IfStatement(next)) => current = next,
                Some(other) => break other,
            }
        };
        let gap = Span::new(current.consequent.span().end, last_else.span().start);
        let span = util::find_token(ctx, gap, "else").unwrap_or_else(|| {
            let start = last_else.span().start;
            Span::new(start, start)
        });
        ctx.emit(
            span,
            "Remove this 'else' and move its body after the 'if': the branches before it always return",
        );
    }
}
