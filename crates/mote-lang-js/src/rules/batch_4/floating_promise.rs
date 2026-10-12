//! `js/floating-promise`

use super::util;
use crate::prelude::*;

pub struct FloatingPromise;

pub static FLOATING_PROMISE: RuleMeta = RuleMeta {
    status: RuleStatus::Stable,
    // Fire-and-forget with a global rejection handler is a deliberate convention in many
    // codebases (about 900 hits on VS Code): opt-in.
    default_enabled: false,
    tags: &["async", "promise"],
    remediation_minutes: 5,
    description: r#"Flags promises that are created and then ignored: a statement that calls an `async`
function defined in the same file, or a `.then(onFulfilled)` / `.finally(…)` chain without a
rejection handler, whose result is neither awaited, returned, nor handled.

**Why:** nothing waits for a floating promise. The code after it runs before the work finishes, and
if the promise rejects, the error is not caught by any surrounding `try`/`catch`: it surfaces as an
unhandled rejection (which terminates Node.js processes by default) or is lost entirely.

**Fix:** `await` the promise, `return` it to the caller, attach a `.catch(…)` handler (or a second
argument to `.then`), or — when running it in the background is intended — mark it with `void`.

```js
async function save(doc) { /* … */ }

save(doc);                        // flagged
fetchUser().then(render);         // flagged: rejections are not handled

await save(doc);                  // compliant
void save(doc);                   // compliant: explicitly fire-and-forget
fetchUser().then(render).catch(showError); // compliant
```

Without type information only promises whose origin is visible are checked: calls to `async`
functions declared in the same file (function declarations and never-reassigned variables holding
an `async` function), and `.then` / `.finally` chains. Chains on Cypress's `cy` object are not
promises and are ignored.
"#,
    ..RuleMeta::new(
        "js/floating-promise",
        "js",
        "Promises should be awaited, returned, or handled",
    )
};

impl JsRule for FloatingPromise {
    fn meta(&self) -> &'static RuleMeta {
        &FLOATING_PROMISE
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::ExpressionStatement]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::ExpressionStatement(stmt) = node.kind() else {
            return;
        };
        let call = match helpers::unwrap_expr(&stmt.expression) {
            Expression::CallExpression(call) => call,
            Expression::ChainExpression(chain) => match &chain.expression {
                ChainElement::CallExpression(call) => call,
                _ => return,
            },
            _ => return,
        };
        // An arrow function's expression body is represented as an expression statement.
        if is_arrow_expression_body(ctx, node) {
            return;
        }
        match helpers::unwrap_expr(&call.callee) {
            Expression::Identifier(id) => {
                if calls_local_async_function(ctx, id) {
                    ctx.emit(
                        util::first_line(ctx, call.span),
                        format!(
                            "Handle the promise returned by '{}': await or return it, add a .catch() handler, or mark it with 'void'",
                            id.name
                        ),
                    );
                }
            }
            Expression::StaticMemberExpression(member) => {
                let unhandled = match member.property.name.as_str() {
                    "then" => call.arguments.len() == 1,
                    "finally" => call.arguments.len() == 1,
                    _ => false,
                };
                if !unhandled || call.arguments.iter().any(|a| a.is_spread()) {
                    return;
                }
                if is_cypress_chain(&member.object) {
                    return;
                }
                ctx.emit(
                    member.property.span,
                    "Handle rejections of this promise chain: add a .catch() handler, await or return it, or mark it with 'void'",
                );
            }
            _ => {}
        }
    }
}

fn is_arrow_expression_body(ctx: &Ctx<'_>, stmt: &AstNode<'_>) -> bool {
    let Some(body) = ctx.parent(stmt) else {
        return false;
    };
    matches!(body.kind(), AstKind::FunctionBody(_))
        && matches!(ctx.parent_kind(body), Some(AstKind::ArrowFunctionExpression(a)) if a.expression)
}

/// The callee resolves to an `async` (non-generator) function declared in this file: a function
/// declaration, or a variable never reassigned that holds an `async` arrow/function expression.
fn calls_local_async_function(ctx: &Ctx<'_>, callee: &IdentifierReference<'_>) -> bool {
    let Some(symbol) = ctx.reference_symbol(callee) else {
        return false;
    };
    if ctx.symbol_flags(symbol).contains(SymbolFlags::Ambient)
        || !ctx.scoping().symbol_redeclarations(symbol).is_empty()
    {
        return false;
    }
    let decl = ctx.symbol_declaration(symbol);
    let body = match decl.kind() {
        AstKind::Function(f) if f.r#async && !f.generator => f.body.as_deref(),
        AstKind::VariableDeclarator(d) => {
            if d.kind == VariableDeclarationKind::Var
                || !matches!(d.id, BindingPattern::BindingIdentifier(_))
            {
                return false;
            }
            match d.init.as_ref().map(helpers::unwrap_expr) {
                Some(Expression::ArrowFunctionExpression(a)) if a.r#async => Some(&*a.body),
                Some(Expression::FunctionExpression(f)) if f.r#async && !f.generator => {
                    f.body.as_deref()
                }
                _ => None,
            }
        }
        _ => None,
    };
    let Some(body) = body else {
        return false;
    };
    !catches_everything(body) && !ctx.references_of(symbol).any(|r| r.is_write())
}

/// `async function f() { try { … } catch { … } }`: the body handles its own errors, so the
/// returned promise does not reject in practice.
fn catches_everything(body: &FunctionBody<'_>) -> bool {
    matches!(body.statements.as_slice(), [Statement::TryStatement(t)] if t.handler.is_some())
}

/// `cy.get(…).then(…)`: Cypress command chains look like promises but are not.
fn is_cypress_chain(expr: &Expression<'_>) -> bool {
    let mut current = helpers::unwrap_expr(expr);
    loop {
        current = match current {
            Expression::CallExpression(call) => helpers::unwrap_expr(&call.callee),
            Expression::StaticMemberExpression(m) => helpers::unwrap_expr(&m.object),
            Expression::ComputedMemberExpression(m) => helpers::unwrap_expr(&m.object),
            Expression::ChainExpression(c) => match &c.expression {
                ChainElement::CallExpression(call) => helpers::unwrap_expr(&call.callee),
                ChainElement::StaticMemberExpression(m) => helpers::unwrap_expr(&m.object),
                ChainElement::ComputedMemberExpression(m) => helpers::unwrap_expr(&m.object),
                _ => return false,
            },
            Expression::Identifier(id) => return id.name == "cy" || id.name == "Cypress",
            _ => return false,
        };
    }
}
