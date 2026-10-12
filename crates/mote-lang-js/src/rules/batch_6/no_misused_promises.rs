//! `js/no-misused-promises`

use super::util;
use crate::prelude::*;

pub struct NoMisusedPromises;

pub static NO_MISUSED_PROMISES: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["async", "promise"],
    remediation_minutes: 5,
    description: r#"Flags promises used where a plain value is expected: a call of an `async` function tested as a
condition, spread into an array or object, or an `async` callback given to an array method that
expects a plain answer (`filter`, `some`, `every`, `find`, `findIndex`, `sort`) or ignores what
the callback returns (`forEach`).

**Why:** a promise is an object, so it is always truthy: `if (isAllowed(user))` is true even when
`isAllowed` resolves to `false`, and `list.filter(async (x) => check(x))` keeps every element.
Spreading a promise copies nothing (`{ ...load() }`) or throws (`[...load()]`). `forEach` does not
wait for an `async` callback: the code after the loop runs before the callbacks finish, and their
rejections are not handled.

**Fix:** `await` the promise first (`if (await isAllowed(user))`), resolve the values before
filtering (`const flags = await Promise.all(list.map(check))`), and replace
`forEach(async …)` with a `for…of` loop that awaits, or with `await Promise.all(list.map(async …))`.

```js
async function isAllowed(user) { /* … */ }

if (isAllowed(user)) grant();                    // flagged: always true
items.forEach(async (item) => await save(item)); // flagged: nothing waits for the saves

if (await isAllowed(user)) grant();              // compliant
for (const item of items) await save(item);      // compliant
```

Without type information only visible promises are checked: calls of `async` functions declared
in the same file (and never reassigned), and `async` function literals passed as callbacks.
"#,
    ..RuleMeta::new(
        "js/no-misused-promises",
        "js",
        "Promises should not be used as conditions, spread, or given to callbacks that ignore them",
    )
};

/// Array methods whose callback result is used as a boolean or a number.
const VALUE_CALLBACKS: &[&str] = &[
    "filter",
    "some",
    "every",
    "find",
    "findIndex",
    "findLast",
    "findLastIndex",
    "sort",
    "toSorted",
];

impl JsRule for NoMisusedPromises {
    fn meta(&self) -> &'static RuleMeta {
        &NO_MISUSED_PROMISES
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CallExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::CallExpression(call) = node.kind() else {
            return;
        };
        match helpers::unwrap_expr(&call.callee) {
            Expression::Identifier(id) => {
                let Some(message) = plain_value_use(ctx, node) else {
                    return;
                };
                if util::is_local_async_function(ctx, id) {
                    ctx.emit(
                        util::first_line(ctx, call.span),
                        format!("'{}' returns a promise: {message}", id.name),
                    );
                }
            }
            Expression::StaticMemberExpression(m) => {
                let method = m.property.name.as_str();
                let value_callback = VALUE_CALLBACKS.contains(&method);
                if !value_callback && method != "forEach" {
                    return;
                }
                let Some(callback) = call.arguments.first().and_then(Argument::as_expression)
                else {
                    return;
                };
                let (is_async, span) = match helpers::unwrap_expr(callback) {
                    Expression::ArrowFunctionExpression(a) => (a.r#async, a.span),
                    Expression::FunctionExpression(f) => (f.r#async && !f.generator, f.span),
                    _ => return,
                };
                if !is_async {
                    return;
                }
                let span = Span::new(span.start, span.start + 5);
                let message = if value_callback {
                    format!(
                        "This async callback returns a promise, which '{method}' treats as always truthy: resolve the values first"
                    )
                } else {
                    "'forEach' does not wait for this async callback: use a for…of loop with await, or Promise.all".to_string()
                };
                ctx.emit(span, message);
            }
            _ => {}
        }
    }
}

/// Is the call's value used as a plain value: a condition (directly, through `!`, or as an operand
/// of `&&`/`||` in a condition) or a spread? Returns the message ending.
fn plain_value_use(ctx: &Ctx<'_>, node: &AstNode<'_>) -> Option<&'static str> {
    const CONDITION: &str = "as a condition it is always true; await it first";
    let mut child = node.span();
    let mut negated = false;
    for ancestor in ctx.ancestors(node) {
        match ancestor.kind() {
            AstKind::TSAsExpression(_)
            | AstKind::TSSatisfiesExpression(_)
            | AstKind::TSNonNullExpression(_)
            | AstKind::TSTypeAssertion(_)
            | AstKind::ParenthesizedExpression(_) => {}
            AstKind::UnaryExpression(u) if u.operator == UnaryOperator::LogicalNot => {
                negated = true;
            }
            AstKind::LogicalExpression(l) if l.operator != LogicalOperator::Coalesce => {
                // `a || load()` as a value returns the promise; only the left operand, or any
                // operand of a logical expression that is itself tested, is a condition.
                if l.left.span() == child {
                    return Some(CONDITION);
                }
            }
            AstKind::IfStatement(s) => return (s.test.span() == child).then_some(CONDITION),
            AstKind::WhileStatement(s) => return (s.test.span() == child).then_some(CONDITION),
            AstKind::DoWhileStatement(s) => return (s.test.span() == child).then_some(CONDITION),
            AstKind::ForStatement(s) => {
                return s
                    .test
                    .as_ref()
                    .is_some_and(|t| t.span() == child)
                    .then_some(CONDITION);
            }
            AstKind::ConditionalExpression(c) => {
                return (c.test.span() == child).then_some(CONDITION);
            }
            AstKind::SpreadElement(_) if !negated => {
                return Some("spreading it copies nothing (or throws); await it first");
            }
            _ => return negated.then_some(CONDITION),
        }
        child = ancestor.span();
    }
    None
}
