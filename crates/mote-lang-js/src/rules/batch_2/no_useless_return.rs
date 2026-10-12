//! `js/no-useless-return`

use crate::prelude::*;

pub struct NoUselessReturn;

pub static NO_USELESS_RETURN: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["redundant"],
    remediation_minutes: 1,
    description: r#"Flags a bare `return;` that is the last statement of a function body, and a `continue;`
that is the last statement of a loop body.

**Why:** a function returns when it reaches its end anyway, and a loop goes on with its next
iteration when its body ends, so the statement does nothing. It makes readers wonder whether
something was meant to be returned, or whether code after it was lost.

**Fix:** remove the `return;` or `continue;`. A `return;` that ends a function early (inside an
`if`, a loop, or followed by other code) is not reported, nor is a final `return;` in a function
that returns a value on other paths, where it documents the `undefined` result, or a `return;`
that is the whole body (an explicit "do nothing"). Likewise a `continue;` inside an `if` of the
loop, a labeled `continue outer;` (which leaves an inner loop), and a loop body made only of
`continue;` are not reported.

```js
function save(item) {
  store.put(item);
  return;             // flagged
}

for (const item of items) {
  store.put(item);
  continue;           // flagged
}

function save(item) {
  if (!item) {
    return;           // compliant: early exit
  }
  store.put(item);
}
```
"#,
    ..RuleMeta::new(
        "js/no-useless-return",
        "js",
        "A bare 'return' should not end a function, nor 'continue' a loop body",
    )
};

/// Functions (by node) that contain a `return <value>`.
#[derive(Default)]
struct ValueReturning(FxHashSet<NodeId>);

impl JsRule for NoUselessReturn {
    fn meta(&self) -> &'static RuleMeta {
        &NO_USELESS_RETURN
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::ReturnStatement, AstType::ContinueStatement]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let ret = match node.kind() {
            AstKind::ReturnStatement(ret) => ret,
            AstKind::ContinueStatement(cont) => {
                check_continue(node, cont, ctx);
                return;
            }
            _ => return,
        };
        if ret.argument.is_some() {
            // Returns are visited in source order, so every `return <value>` of a function is
            // recorded before its trailing `return;`.
            if let Some(function) = ctx.enclosing_function(node) {
                ctx.state::<ValueReturning>().0.insert(function.id());
            }
            return;
        }
        let Some(body_node) = ctx.parent(node) else {
            return;
        };
        let AstKind::FunctionBody(body) = body_node.kind() else {
            return;
        };
        if body.statements.last().map(GetSpan::span) != Some(ret.span) {
            return;
        }
        // `{ return; }` alone is a deliberate, explicit "do nothing" body.
        if body.statements.len() == 1 {
            return;
        }
        // In a function that returns values elsewhere, an explicit final `return;` documents the
        // `undefined` result (and satisfies TypeScript's `noImplicitReturns`).
        let Some(function) = ctx.parent(body_node) else {
            return;
        };
        if ctx.state::<ValueReturning>().0.contains(&function.id()) {
            return;
        }
        ctx.emit(ret.span, "Remove this redundant 'return'");
    }
}

/// `for (…) { work(); continue; }`: the loop goes on with its next iteration anyway.
fn check_continue<'a>(node: &AstNode<'a>, cont: &ContinueStatement<'a>, ctx: &mut Ctx<'a>) {
    // `continue outer;` at the end of an inner loop leaves the inner loop: not redundant.
    if cont.label.is_some() {
        return;
    }
    let Some(block_node) = ctx.parent(node) else {
        return;
    };
    let AstKind::BlockStatement(block) = block_node.kind() else {
        return;
    };
    // `{ continue; }` alone is an explicit "nothing to do" body.
    if block.body.len() < 2 || block.body.last().map(GetSpan::span) != Some(cont.span) {
        return;
    }
    let is_loop_body = match ctx.parent_kind(block_node) {
        Some(AstKind::ForStatement(s)) => s.body.span() == block.span,
        Some(AstKind::ForInStatement(s)) => s.body.span() == block.span,
        Some(AstKind::ForOfStatement(s)) => s.body.span() == block.span,
        Some(AstKind::WhileStatement(s)) => s.body.span() == block.span,
        Some(AstKind::DoWhileStatement(s)) => s.body.span() == block.span,
        _ => false,
    };
    if is_loop_body {
        ctx.emit(cont.span, "Remove this redundant 'continue'");
    }
}
