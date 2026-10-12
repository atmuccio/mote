//! `js/skipped-test-without-reason`

use super::util;
use crate::prelude::*;

pub struct SkippedTestWithoutReason;

pub static SKIPPED_TEST_WITHOUT_REASON: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["tests"],
    remediation_minutes: 2,
    description: r#"Flags skipped tests and suites (`it.skip`, `test.skip`, `describe.skip`, `xit`, `xdescribe`, …)
with no comment saying why.

**Why:** a skipped test silently stops protecting the code. Without a reason nobody knows whether
it is flaky, waiting for a fix, or obsolete, so it tends to stay skipped for good, and the next
person to touch it cannot tell whether turning it back on is safe.

**Fix:** say why on the line of the skip or just above it (ideally with an issue reference), or
delete the test if it is no longer relevant.

```js
it.skip('imports a large file', async () => { … });   // flagged

// Flaky on Windows CI, see #1234
it.skip('imports a large file', async () => { … });   // compliant
```

A comment on the same line, on the line directly above, or at the top of the test's body counts
as the reason, and so does a title that states one (`'TODO …'`, `'flaky: …'`, an issue number or link). Not reported:
skips decided at run time (`test.skip(condition, 'reason')`, `this.skip()`, `it.skipIf(…)`),
`todo` tests, skipped tests inside a suite that is itself skipped (the suite is reported), and
files that are not tests. Only the test globals and imported test functions are
checked.
"#,
    ..RuleMeta::new(
        "js/skipped-test-without-reason",
        "js",
        "Skipped tests should say why they are skipped",
    )
};

const X_FUNCTIONS: &[&str] = &[
    "xit",
    "xtest",
    "xdescribe",
    "xcontext",
    "xspecify",
    "xsuite",
];

impl JsRule for SkippedTestWithoutReason {
    fn meta(&self) -> &'static RuleMeta {
        &SKIPPED_TEST_WITHOUT_REASON
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
        // `xit(…)`, or `.skip` in `it.skip(…)` / `test.describe.skip(…)`.
        let root = match helpers::unwrap_expr(&call.callee) {
            Expression::Identifier(id) if X_FUNCTIONS.contains(&id.name.as_str()) => &**id,
            callee => {
                // The root name first: collecting the chain's member names allocates.
                if !util::callee_root(callee)
                    .is_some_and(|root| util::TEST_FUNCTIONS.contains(&root.name.as_str()))
                {
                    return;
                }
                let Some((root, members)) = util::callee_chain(callee) else {
                    return;
                };
                if !members.contains(&"skip") {
                    return;
                }
                // Only the call that registers the test: `it.skip(name, fn)`, not the table
                // forms (`it.skip.each(table)(name, fn)`).
                if !matches!(callee, Expression::StaticMemberExpression(m) if m.property.name != "each")
                {
                    return;
                }
                root
            }
        };
        if !util::is_test_api(ctx, root) {
            return;
        }
        // `test.skip(condition, 'reason')` and `test.skip()` inside a test decide at run time.
        let Some(first) = call.arguments.first().and_then(Argument::as_expression) else {
            return;
        };
        if !matches!(
            helpers::unwrap_expr(first),
            Expression::StringLiteral(_) | Expression::TemplateLiteral(_)
        ) {
            return;
        }
        // A title that already says why (`'flaky: …'`, `'TODO …'`, an issue number).
        if title_gives_reason(ctx.text(first.span())) {
            return;
        }
        if has_reason_comment(ctx, call.span) || has_leading_body_comment(ctx, call) {
            return;
        }
        // Inside a suite that is skipped already: the suite's finding covers it.
        if ctx.ancestors(node).any(|a| match a.kind() {
            AstKind::CallExpression(outer) => is_skip_callee(&outer.callee),
            _ => false,
        }) {
            return;
        }
        ctx.emit(
            util::first_line(ctx, first.span()),
            format!(
                "Say why '{}' is skipped in a comment, or remove it",
                ctx.text(call.callee.span())
            ),
        );
    }
}

fn is_skip_callee(callee: &Expression<'_>) -> bool {
    match helpers::unwrap_expr(callee) {
        Expression::Identifier(id) => X_FUNCTIONS.contains(&id.name.as_str()),
        callee => {
            util::callee_root(callee)
                .is_some_and(|root| util::TEST_FUNCTIONS.contains(&root.name.as_str()))
                && util::callee_chain(callee).is_some_and(|(_, members)| members.contains(&"skip"))
        }
    }
}

fn title_gives_reason(title: &str) -> bool {
    let lower = title.to_ascii_lowercase();
    [
        "todo", "fixme", "flaky", "broken", "disabled", "skip", "wip", "http", "issue", "bug",
    ]
    .iter()
    .any(|w| lower.contains(w))
        || lower
            .as_bytes()
            .windows(2)
            .any(|w| w[0] == b'#' && w[1].is_ascii_digit())
}

/// A comment at the top of the test's callback, before its first statement (or anywhere in an
/// empty body): `it.skip('…', () => { // flaky on CI …`.
fn has_leading_body_comment(ctx: &Ctx<'_>, call: &CallExpression<'_>) -> bool {
    let body =
        call.arguments
            .iter()
            .rev()
            .find_map(|a| match helpers::unwrap_expr(a.as_expression()?) {
                Expression::ArrowFunctionExpression(f) if !f.expression => Some(&*f.body),
                Expression::FunctionExpression(f) => f.body.as_deref(),
                _ => None,
            });
    let Some(body) = body else {
        return false;
    };
    let end = body
        .statements
        .first()
        .map_or(body.span.end, |s| s.span().start);
    ctx.has_comments_in(Span::new(body.span.start, end))
}

/// A comment on the first line of the call, or ending on the line just above it.
fn has_reason_comment(ctx: &Ctx<'_>, span: Span) -> bool {
    let line = ctx.line_of(span.start);
    ctx.comments().iter().any(|c| {
        let start = ctx.line_of(c.span.start);
        let end = ctx.line_of(c.span.end);
        start == line || end == line || end + 1 == line
    })
}
