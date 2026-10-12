//! `js/no-fallthrough`

use super::util;
use crate::prelude::*;

pub struct NoFallthrough;

pub static NO_FALLTHROUGH: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["suspicious"],
    remediation_minutes: 5,
    description: r#"Flags `switch` cases that run into the next case because they do not end with `break`,
`return`, `throw`, or `continue`.

**Why:** falling through is legal but rarely intended; a forgotten `break` makes the next case's
code run too, which is a classic source of subtle bugs. When falling through *is* intended, readers
need to be told.

**Fix:** end the case with `break` (or `return`/`throw`), or — if the fall-through is deliberate —
put a comment such as `// falls through` right before the next case.

```js
switch (level) {
  case 'debug':
    enableTracing();
  case 'info':               // flagged: 'debug' falls into 'info'
    enableLogging();
    break;
  case 'warn':
    notify();
    // falls through
  case 'error':              // compliant: documented
    alert();
}
```

Empty cases that share a body (`case 'a': case 'b': …`) are fine, and so are comments such as
`// fall through`, `/* fallthrough */`, `// no break`, or `// intentionally …` between the two cases.
Cases ending in an infinite loop, a nested `switch`, `process.exit()`, `assert(false)`, or a call
whose name says it never returns (`fail…`, `throw…`, `assertNever…`, `unreachable…`) are not
reported. The finding is reported on the case that is fallen into.
"#,
    ..RuleMeta::new(
        "js/no-fallthrough",
        "js",
        "Switch cases should not fall through unintentionally",
    )
};

impl JsRule for NoFallthrough {
    fn meta(&self) -> &'static RuleMeta {
        &NO_FALLTHROUGH
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::SwitchStatement]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::SwitchStatement(switch) = node.kind() else {
            return;
        };
        for pair in switch.cases.windows(2) {
            let [case, next] = pair else {
                continue;
            };
            let Some(last) = case.consequent.last() else {
                continue;
            };
            if util::list_exits(&case.consequent) || may_not_complete(last) {
                continue;
            }
            let gap = Span::new(last.span().start, next.span.start);
            if has_fallthrough_comment(ctx, gap) {
                continue;
            }
            let keyword = if next.test.is_some() {
                "case"
            } else {
                "default"
            };
            let span = util::keyword_span(ctx, next.span, keyword);
            ctx.emit(
                span,
                format!("Add a 'break' before this '{keyword}', or a '// falls through' comment if falling through is intended"),
            );
        }
    }
}

/// Statements whose normal completion we cannot rule out cheaply but that usually do not
/// complete: infinite loops, nested `switch` statements, labeled blocks, and calls to functions
/// that by name never return.
fn may_not_complete(stmt: &Statement<'_>) -> bool {
    match stmt {
        Statement::WhileStatement(w) => is_true(&w.test),
        Statement::DoWhileStatement(w) => is_true(&w.test),
        Statement::ForStatement(f) => f.test.as_ref().is_none_or(is_true),
        Statement::SwitchStatement(_) | Statement::LabeledStatement(_) => true,
        Statement::BlockStatement(b) => b.body.last().is_some_and(may_not_complete),
        Statement::ExpressionStatement(e) => is_never_returning_call(&e.expression),
        _ => false,
    }
}

fn is_true(expr: &Expression<'_>) -> bool {
    match helpers::unwrap_expr(expr) {
        Expression::BooleanLiteral(b) => b.value,
        Expression::NumericLiteral(n) => n.value != 0.0,
        _ => false,
    }
}

/// `process.exit(…)`, `assertNever(x)`, `this.fail(…)`, `throwError(…)`, `unreachable()`, …
fn is_never_returning_call(expr: &Expression<'_>) -> bool {
    let Expression::CallExpression(call) = helpers::unwrap_expr(expr) else {
        return false;
    };
    let name = match helpers::unwrap_expr(&call.callee) {
        Expression::Identifier(id) => id.name.as_str(),
        Expression::StaticMemberExpression(m) => {
            if m.property.name == "exit" {
                return true;
            }
            m.property.name.as_str()
        }
        _ => return false,
    };
    // `assert(false)`, `assert.ok(false, …)`: an assertion that always fails.
    let first_is_false = call
        .arguments
        .first()
        .and_then(Argument::as_expression)
        .is_some_and(
            |a| matches!(helpers::unwrap_expr(a), Expression::BooleanLiteral(b) if !b.value),
        );
    if first_is_false && (name == "assert" || name == "ok") {
        return true;
    }
    let lower = |prefix: &str| {
        name.get(..prefix.len())
            .is_some_and(|p| p.eq_ignore_ascii_case(prefix))
    };
    lower("throw")
        || lower("fail")
        || lower("assertnever")
        || lower("assertunreachable")
        || lower("unreachable")
        || lower("panic")
        || lower("abort")
}

/// A comment like `// falls through`, `/* fallthrough */`, `// fall-thru`, `// no break`, or
/// `// intentionally …` in the gap.
fn has_fallthrough_comment(ctx: &Ctx<'_>, gap: Span) -> bool {
    if !ctx.has_comments_in(gap) {
        return false;
    }
    let comments = ctx.comments();
    let first = comments.partition_point(|c| c.span.start < gap.start);
    comments[first..]
        .iter()
        .take_while(|c| c.span.end <= gap.end)
        .any(|c| is_fallthrough_text(ctx.text(c.span)))
}

fn is_fallthrough_text(text: &str) -> bool {
    let mut squashed = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_alphanumeric() {
            squashed.extend(c.to_lowercase());
        }
    }
    [
        "fallthrough",
        "fallsthrough",
        "fallthru",
        "fallsthru",
        "fallingthrough",
        "nobreak",
        "notbreak",
        "dontbreak",
        "intentional",
    ]
    .iter()
    .any(|p| squashed.contains(p))
}
