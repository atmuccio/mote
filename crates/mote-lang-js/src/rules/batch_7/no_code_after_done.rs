//! `js/no-code-after-done`

use crate::prelude::*;

pub struct NoCodeAfterDone;

pub static NO_CODE_AFTER_DONE: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["tests", "suspicious"],
    remediation_minutes: 5,
    description: r#"Flags assertions (and further `done` calls) that run after a callback-style test has called
`done()`: statements after `done()` in the same block, or after an `if (…) done(…)` without
`else` or `return`.

**Why:** calling `done` tells the test runner that the test is over. An assertion that runs
afterwards is no longer part of this test: when it fails, the failure is reported against
another test, turns into an unhandled error, or is lost — the test passes either way. Calling
`done` a second time is an error of its own in most runners.

**Fix:** make `done()` the last thing the test does — move the assertions before it — and
`return` after `done(error)` in error branches.

```js
it('loads', (done) => {
  load((err, data) => {
    if (err) done(err);          // flagged: execution continues below
    done();
    expect(data).toBeDefined(); // flagged: runs after the test ended
  });
});

it('loads', (done) => {
  load((err, data) => {
    if (err) return done(err);
    expect(data).toBeDefined();
    done();                     // compliant
  });
});
```

Only the first parameter of a function passed to `it`, `test`, `specify` or a
`before…`/`after…` hook counts as `done`, and only statements that contain an assertion
(`expect`, `assert`, `should`) or another `done` call are reported — plain clean-up after `done()`
is harmless.
"#,
    ..RuleMeta::new(
        "js/no-code-after-done",
        "js",
        "Assertions should not run after a test's 'done' callback is called",
    )
};

impl JsRule for NoCodeAfterDone {
    fn meta(&self) -> &'static RuleMeta {
        &NO_CODE_AFTER_DONE
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CallExpression]
    }

    fn callee_names(&self) -> &'static [&'static str] {
        &[
            "it",
            "test",
            "specify",
            "before",
            "after",
            "beforeEach",
            "afterEach",
            "beforeAll",
            "afterAll",
            "only",
            "skip",
        ]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::CallExpression(call) = node.kind() else {
            return;
        };
        if !is_test_function_call(ctx, &call.callee) {
            return;
        }
        let Some(callback) = call.arguments.last().and_then(Argument::as_expression) else {
            return;
        };
        let params = match helpers::unwrap_expr(callback) {
            Expression::ArrowFunctionExpression(f) => &f.params,
            Expression::FunctionExpression(f) => &f.params,
            _ => return,
        };
        let Some(BindingPattern::BindingIdentifier(done)) =
            params.items.first().map(|p| &p.pattern)
        else {
            return;
        };
        let Some(done_symbol) = ctx.binding_symbol(done) else {
            return;
        };
        let mut found: Vec<(Span, Span)> = Vec::new();
        for reference in ctx.references_of(done_symbol) {
            let at = ctx.node(reference.node_id());
            let Some(statement) = done_statement(ctx, at) else {
                continue;
            };
            if let Some(late) = assertion_after(ctx, statement, done_symbol) {
                found.push((statement.span(), late));
            }
        }
        for (done_span, late) in found {
            ctx.emit(
                super::super::batch_6::util::first_line(ctx, late),
                "This runs after 'done()' was called, when the test is already over: move it before 'done()' (or 'return' after it)",
            )
            .secondary(done_span, "'done' is called here");
        }
    }
}

/// `it`, `test`, `specify`, hooks, and their `.only` / `.skip` forms, as globals.
fn is_test_function_call(ctx: &Ctx<'_>, callee: &Expression<'_>) -> bool {
    let id = match helpers::unwrap_expr(callee) {
        Expression::Identifier(id) => id,
        Expression::StaticMemberExpression(m)
            if matches!(m.property.name.as_str(), "only" | "skip") =>
        {
            match helpers::unwrap_expr(&m.object) {
                Expression::Identifier(id) => id,
                _ => return false,
            }
        }
        _ => return false,
    };
    matches!(
        id.name.as_str(),
        "it" | "test"
            | "specify"
            | "before"
            | "after"
            | "beforeEach"
            | "afterEach"
            | "beforeAll"
            | "afterAll"
    ) && ctx.is_global_reference(id)
}

/// The statement `done(…);` / `done.fail(…);` / `return done(…)`-less call that this reference
/// makes, when the call is a whole expression statement.
fn done_statement<'a>(ctx: &Ctx<'a>, reference: &AstNode<'a>) -> Option<&'a AstNode<'a>> {
    let mut call = ctx.parent(reference)?;
    if let AstKind::StaticMemberExpression(m) = call.kind() {
        if m.property.name != "fail" {
            return None;
        }
        call = ctx.parent(call)?;
    }
    let AstKind::CallExpression(c) = call.kind() else {
        return None;
    };
    // The reference must be the callee, not an argument (`cb(done)`).
    let callee = helpers::unwrap_expr(&c.callee).span();
    if !(callee.start <= reference.span().start && reference.span().end <= callee.end) {
        return None;
    }
    let statement = ctx.outer_parent(call)?;
    matches!(statement.kind(), AstKind::ExpressionStatement(_)).then_some(statement)
}

/// The first later statement that asserts or calls `done` again, in the statement list that holds
/// `statement` — or, when `statement` ends the brace-less or block body of an `if` without
/// `else`, in the list that holds that `if`.
fn assertion_after(ctx: &Ctx<'_>, statement: &AstNode<'_>, done: SymbolId) -> Option<Span> {
    let mut current = statement;
    for _ in 0..2 {
        let parent = ctx.parent(current)?;
        let (list, is_block_of_if): (&[Statement<'_>], bool) = match parent.kind() {
            AstKind::BlockStatement(b) => (
                &b.body,
                matches!(ctx.parent_kind(parent), Some(AstKind::IfStatement(i)) if i.alternate.is_none() && i.consequent.span() == b.span),
            ),
            AstKind::FunctionBody(b) => (&b.statements, false),
            AstKind::SwitchCase(c) => (&c.consequent, false),
            AstKind::IfStatement(i)
                if i.alternate.is_none() && i.consequent.span() == current.span() =>
            {
                // `if (err) done(err);` — continue with what follows the `if`.
                current = parent;
                continue;
            }
            _ => return None,
        };
        let at = current.span();
        let index = list.iter().position(|s| s.span() == at)?;
        let rest = &list[index + 1..];
        // Code after a `return` (or `break`, `continue`, `throw`) does not run.
        let reachable = rest
            .iter()
            .position(|s| {
                matches!(
                    s,
                    Statement::ReturnStatement(_)
                        | Statement::BreakStatement(_)
                        | Statement::ContinueStatement(_)
                        | Statement::ThrowStatement(_)
                )
            })
            .map_or(rest, |i| &rest[..i]);
        if let (Some(first), Some(last)) = (reachable.first(), reachable.last()) {
            let window = Span::new(first.span().start, last.span().end);
            if let Some(hit) = first_assertion(ctx, current, window, done) {
                return reachable
                    .iter()
                    .find(|s| s.span().start <= hit && hit < s.span().end)
                    .map(GetSpan::span);
            }
        }
        // A trailing `done()` in an `if` without `else`: look after the `if`.
        if is_block_of_if && rest.is_empty() {
            current = ctx.parent(parent)?;
            continue;
        }
        return None;
    }
    None
}

/// Offset of the first assertion or `done` call inside `window`, which follows `after` in the same
/// statement list. The nodes of the following statements come right after `after`'s subtree in
/// traversal order, so only they are visited.
fn first_assertion(
    ctx: &Ctx<'_>,
    after: &AstNode<'_>,
    window: Span,
    done: SymbolId,
) -> Option<u32> {
    let nodes = ctx.nodes();
    let skip = after.span().end;
    (after.id().index() + 1..nodes.len())
        .map(|i| nodes.get_node(NodeId::new(i)))
        .skip_while(|n| n.span().start < skip)
        .take_while(|n| n.span().start < window.end)
        .filter(|n| n.span().start >= window.start)
        .find(|n| is_assertion_or_done(ctx, n, done))
        .map(|n| n.span().start)
}

/// A call of `expect`, `assert`, `assert.x`, `done`, `done.fail`, or a `.should` chain.
fn is_assertion_or_done(ctx: &Ctx<'_>, node: &AstNode<'_>, done: SymbolId) -> bool {
    match node.kind() {
        AstKind::CallExpression(call) => match helpers::unwrap_expr(&call.callee) {
            Expression::Identifier(id) => {
                matches!(id.name.as_str(), "expect" | "assert")
                    || ctx.reference_symbol(id) == Some(done)
            }
            Expression::StaticMemberExpression(m) => match helpers::unwrap_expr(&m.object) {
                Expression::Identifier(id) => {
                    id.name == "assert"
                        || (m.property.name == "fail" && ctx.reference_symbol(id) == Some(done))
                }
                _ => false,
            },
            _ => false,
        },
        AstKind::StaticMemberExpression(m) => m.property.name == "should",
        _ => false,
    }
}
