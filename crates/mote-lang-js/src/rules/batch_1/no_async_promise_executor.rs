//! `js/no-async-promise-executor`

use crate::prelude::*;

pub struct NoAsyncPromiseExecutor;

pub static NO_ASYNC_PROMISE_EXECUTOR: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["async", "error-handling"],
    remediation_minutes: 10,
    description: r#"Flags `new Promise(async (resolve, reject) => { … })` — an `async` function passed as the
executor of a promise.

**Why:** the `Promise` constructor ignores the promise returned by the executor. If the async
executor throws (or an awaited call rejects), the error goes nowhere: the new promise never
settles and the rejection is unhandled. It also signals that the wrapper is unnecessary, since an
async function already returns a promise.

**Fix:** remove the `new Promise` wrapper and use the async function directly, or make the
executor synchronous and call `reject` on errors.

```js
const data = new Promise(async (resolve) => {   // flagged
  resolve(await load());
});

const data = (async () => load())();            // compliant
const data = new Promise((resolve, reject) => { // compliant
  load().then(resolve, reject);
});
```
"#,
    ..RuleMeta::new(
        "js/no-async-promise-executor",
        "js",
        "Promise executors should not be async functions",
    )
};

impl JsRule for NoAsyncPromiseExecutor {
    fn meta(&self) -> &'static RuleMeta {
        &NO_ASYNC_PROMISE_EXECUTOR
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::NewExpression]
    }

    fn callee_names(&self) -> &'static [&'static str] {
        &["Promise"]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::NewExpression(new) = node.kind() else {
            return;
        };
        if !ctx.is_global(&new.callee, "Promise") {
            return;
        }
        let Some(executor) = new.arguments.first().and_then(Argument::as_expression) else {
            return;
        };
        let (is_async, span) = match helpers::unwrap_expr(executor) {
            Expression::ArrowFunctionExpression(f) => (f.r#async, f.span),
            Expression::FunctionExpression(f) => (f.r#async, f.span),
            _ => return,
        };
        if !is_async {
            return;
        }
        let span = if ctx.text(span).starts_with("async") {
            Span::new(span.start, span.start + 5)
        } else {
            span
        };
        ctx.emit(
            span,
            "Remove 'async' from this Promise executor: errors it throws are lost and the promise never settles",
        );
    }
}
