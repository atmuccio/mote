//! `js/test-retries`

use super::util;
use crate::prelude::*;

pub struct TestRetries;

pub static TEST_RETRIES: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Incubating,
    tags: &["tests", "flaky"],
    remediation_minutes: 30,
    description: r#"Flags retries configured inside test code: `jest.retryTimes(n)`, Mocha's `this.retries(n)` and
`it(…).retries(n)`, a `retry`/`retries` option passed to a test or suite (`it('…', { retry: 3 }, …)`),
and Playwright's `test.describe.configure({ retries: n })`.

**Why:** retrying a test until it passes hides the flakiness instead of fixing it. The race or the
leaked state behind it stays in the code (and often in the product), the suite gets slower, and a
real regression that only fails sometimes now passes on the second try.

**Fix:** find and remove the source of the flakiness: wait for the condition instead of a fixed
time, isolate shared state, stub the unstable dependency. If an external system really is
unreliable, retry the call to it, not the whole test.

```js
jest.retryTimes(3);                                  // flagged
it('syncs', { retry: 2 }, async () => { … });        // flagged

it('syncs', async () => {                            // compliant
  await waitFor(() => expect(store.synced).toBe(true));
});
```

Not reported: a retry count of `0` (turning retries off), retries configured for the whole run
in the runner's configuration file (a CI policy, not test code), and files that are not tests.
"#,
    ..RuleMeta::new(
        "js/test-retries",
        "js",
        "Tests should not be retried to hide flakiness",
    )
};

impl JsRule for TestRetries {
    fn meta(&self) -> &'static RuleMeta {
        &TEST_RETRIES
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CallExpression]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.is_test()
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::CallExpression(call) = node.kind() else {
            return;
        };
        match helpers::unwrap_expr(&call.callee) {
            Expression::StaticMemberExpression(m) => {
                let name = m.property.name.as_str();
                let object = helpers::unwrap_expr(&m.object);
                let is_retry_call = match name {
                    // `jest.retryTimes(3)`
                    "retryTimes" => {
                        matches!(object, Expression::Identifier(id) if id.name == "jest" && util::is_test_api(ctx, id))
                    }
                    // `this.retries(3)` (Mocha) and `it(…).retries(3)`.
                    "retries" => match object {
                        Expression::ThisExpression(_) => in_mocha_callback(ctx, node),
                        Expression::CallExpression(c) => {
                            util::callee_root(&c.callee).is_some_and(|root| {
                                util::TEST_FUNCTIONS.contains(&root.name.as_str())
                                    && util::is_test_api(ctx, root)
                            })
                        }
                        _ => false,
                    },
                    // `test.describe.configure({ retries: 2 })` (Playwright).
                    "configure" => {
                        let is_playwright =
                            util::callee_chain(&m.object).is_some_and(|(root, members)| {
                                root.name == "test"
                                    && members == ["describe"]
                                    && util::is_test_api(ctx, root)
                            });
                        if !is_playwright {
                            return;
                        }
                        if let Some(Expression::ObjectExpression(o)) =
                            call.arguments.first().and_then(Argument::as_expression)
                        {
                            report_option(ctx, o);
                        }
                        return;
                    }
                    _ => false,
                };
                if is_retry_call && !disables(call.arguments.first()) {
                    ctx.emit(
                        m.property.span,
                        format!("Fix the flaky test instead of retrying it with '{name}'"),
                    );
                    return;
                }
                if is_retry_call {
                    return;
                }
                self.check_options(call, ctx);
            }
            Expression::Identifier(_) => self.check_options(call, ctx),
            _ => {}
        }
    }
}

impl TestRetries {
    /// `it('…', { retry: 3 }, fn)` / `test('…', fn, { retry: 3 })` / `describe('…', { retries: 2 }, fn)`.
    fn check_options<'a>(&self, call: &CallExpression<'a>, ctx: &mut Ctx<'a>) {
        let Some(root) = util::callee_root(&call.callee) else {
            return;
        };
        if !util::TEST_FUNCTIONS.contains(&root.name.as_str()) || !util::is_test_api(ctx, root) {
            return;
        }
        for arg in call.arguments.iter().skip(1) {
            if let Some(Expression::ObjectExpression(o)) = arg.as_expression() {
                report_option(ctx, o);
            }
        }
    }
}

/// Is `this` the Mocha context: inside a function passed to `describe`, `it` or a hook?
fn in_mocha_callback(ctx: &Ctx<'_>, node: &AstNode<'_>) -> bool {
    let Some(function) = ctx.enclosing_function(node) else {
        return false;
    };
    if !matches!(function.kind(), AstKind::Function(_)) {
        return false;
    }
    let Some(AstKind::CallExpression(call)) = ctx.outer_parent(function).map(AstNode::kind) else {
        return false;
    };
    util::callee_root(&call.callee).is_some_and(|root| {
        matches!(
            root.name.as_str(),
            "describe"
                | "context"
                | "suite"
                | "it"
                | "test"
                | "specify"
                | "before"
                | "beforeEach"
                | "after"
                | "afterEach"
        ) && util::is_test_api(ctx, root)
    })
}

fn report_option<'a>(ctx: &mut Ctx<'a>, object: &ObjectExpression<'a>) {
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(p) = property else {
            continue;
        };
        let Some(key) = p.key.static_name() else {
            continue;
        };
        if key != "retry" && key != "retries" {
            continue;
        }
        if helpers::number_value(&p.value) == Some(0.0) {
            continue;
        }
        ctx.emit(
            p.key.span(),
            format!("Fix the flaky test instead of retrying it with '{key}'"),
        );
    }
}

/// `retryTimes(0)` / `retries(0)` turn retries off.
fn disables(arg: Option<&Argument<'_>>) -> bool {
    arg.and_then(Argument::as_expression)
        .and_then(helpers::number_value)
        == Some(0.0)
}
