//! `js/unawaited-promise-in-try`

use super::util;
use crate::prelude::*;

pub struct UnawaitedPromiseInTry;

pub static UNAWAITED_PROMISE_IN_TRY: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["async", "promise", "error-handling"],
    remediation_minutes: 5,
    description: r#"Flags a promise inside the `try` block of an `async` function that is returned or left running
without `await`, so the surrounding `catch` never sees its rejection (and a `finally` block runs
before it settles).

**Why:** `try`/`catch` only catches what happens while the `try` block runs. `return promise`
hands the promise to the caller and leaves the block at once: if the promise rejects later, the
error goes straight to the caller and the `catch` block written for it is skipped; a `finally`
block has already released whatever the promise still needs. A promise started without `await`
as the only statement of a `try` block escapes the same way, and its rejection is not handled at
all.

**Fix:** `return await promise;` (or `await promise;`), so that the rejection is raised inside the
`try` block.

```js
async function load(id) {
  try {
    return fetchUser(id);            // flagged: a rejection skips the catch below
  } catch (err) {
    return fallbackUser(id);
  }
}

async function load(id) {
  try {
    return await fetchUser(id);      // compliant
  } catch (err) {
    return fallbackUser(id);
  }
}
```

Without type information only promises whose origin is visible are checked: calls of `async`
functions and methods declared in the same file (and TypeScript functions declared to return a
`Promise`), `.then` / `.finally` chains, `new Promise(…)`, `Promise.all` and its siblings, and
`fetch(…)`. A chain ending in `.catch(…)` (or `.then` with a rejection handler) handles its own
rejection and is not reported, nor is a `catch` block that only rethrows, nor a call of an `async`
function whose body is a `try`/`catch` whose `catch` block does not throw, possibly after setup
statements that neither await, throw, nor return anything but plain values (it does not reject in
practice). Not reported either: a call of the enclosing function itself (a retry: the inner call
runs the same `try` with its own `catch` and `finally`, and awaiting it would handle its failures
again at every level, or hold a lock the inner level waits for), and, against a `catch` block
only, a `new Promise` whose executor takes no `reject` callback (a `finally` block still runs too
early for it). A promise started without
`await` next to other statements in the `try` block is left to `js/floating-promise` (the `catch`
may well be meant for the other statements). Only `async` functions
are checked (elsewhere `await` is not available), and async generators, which await returned
values on their own, are skipped.
"#,
    ..RuleMeta::new(
        "js/unawaited-promise-in-try",
        "js",
        "Promises inside a try block should be awaited",
    )
};

impl JsRule for UnawaitedPromiseInTry {
    fn meta(&self) -> &'static RuleMeta {
        &UNAWAITED_PROMISE_IN_TRY
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::ReturnStatement, AstType::ExpressionStatement]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.file_contains(AstType::TryStatement)
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let (expr, returned) = match node.kind() {
            AstKind::ReturnStatement(ret) => match &ret.argument {
                Some(arg) => (arg, true),
                None => return,
            },
            AstKind::ExpressionStatement(stmt) => {
                if util::is_arrow_expression_body(ctx, node) {
                    return;
                }
                (&stmt.expression, false)
            }
            _ => return,
        };
        let expr = helpers::unwrap_expr(expr);
        if !matches!(
            expr,
            Expression::CallExpression(_) | Expression::NewExpression(_)
        ) {
            return;
        }
        let Some((try_stmt, function)) = enclosing_try(ctx, node) else {
            return;
        };
        // A statement: only when it is the whole `try` block, so the `catch` is clearly for it.
        if !returned
            && !matches!(try_stmt.block.body.as_slice(), [only] if only.span() == node.span())
        {
            return;
        }
        // The `catch` block matters only for a promise that can reject.
        let catches =
            try_stmt.handler.as_ref().is_some_and(|h| !only_rethrows(h)) && !never_rejects(expr);
        let has_finally = try_stmt.finalizer.is_some();
        if !(catches || returned && has_finally) {
            return;
        }
        if !is_promise(ctx, expr, node) || handles_rejection(expr) {
            return;
        }
        // A retry through the function itself: the inner call runs the same `try` with its own
        // `catch` and `finally`; awaiting it would handle its failures again at every level (and
        // hold this level's resources while the inner level waits for them).
        if is_self_call(ctx, expr, function) {
            return;
        }
        let span = util::first_line(ctx, expr.span());
        let message = if !returned {
            "Await this promise: otherwise its rejection escapes the surrounding catch block"
        } else if catches {
            "Await this promise before returning it: otherwise its rejection skips the surrounding catch block"
        } else {
            "Await this promise before returning it: otherwise the finally block runs before it settles"
        };
        ctx.emit(span, message);
    }
}

/// The `try` statement whose protected block contains `node`, inside an `async` (non-generator)
/// function and with no function boundary in between; and that function.
fn enclosing_try<'a>(
    ctx: &Ctx<'a>,
    node: &AstNode<'a>,
) -> Option<(&'a TryStatement<'a>, &'a AstNode<'a>)> {
    let mut found = None;
    let mut child = node.span();
    for ancestor in ctx.ancestors(node) {
        match ancestor.kind() {
            AstKind::TryStatement(t) => {
                if found.is_none() && t.block.span == child {
                    found = Some(t);
                }
            }
            AstKind::Function(f) => {
                return found
                    .filter(|_| f.r#async && !f.generator)
                    .map(|t| (t, ancestor));
            }
            AstKind::ArrowFunctionExpression(a) => {
                return found.filter(|_| a.r#async).map(|t| (t, ancestor));
            }
            AstKind::Class(_) | AstKind::StaticBlock(_) | AstKind::Program(_) => return None,
            _ => {}
        }
        child = ancestor.span();
    }
    None
}

/// `catch (e) { throw e; }`: nothing is lost when the rejection skips it.
fn only_rethrows(handler: &CatchClause<'_>) -> bool {
    let Some(param) = &handler.param else {
        return false;
    };
    let BindingPattern::BindingIdentifier(id) = &param.pattern else {
        return false;
    };
    matches!(handler.body.body.as_slice(), [Statement::ThrowStatement(t)]
        if matches!(helpers::unwrap_expr(&t.argument), Expression::Identifier(r) if r.name == id.name))
}

/// `new Promise((resolve) => …)`: an executor that takes no `reject` callback settles only by
/// resolving (a synchronous throw inside it is not the failure a `catch` around it is written for).
fn never_rejects(expr: &Expression<'_>) -> bool {
    let Expression::NewExpression(new) = expr else {
        return false;
    };
    let params = match new.arguments.first().and_then(Argument::as_expression) {
        Some(Expression::ArrowFunctionExpression(a)) => &a.params,
        Some(Expression::FunctionExpression(f)) => &f.params,
        _ => return false,
    };
    params.items.len() <= 1 && params.rest.is_none()
}

/// `return retry(n - 1)` inside `retry` itself, or `return this.poll()` inside the method `poll`.
fn is_self_call(ctx: &Ctx<'_>, expr: &Expression<'_>, function: &AstNode<'_>) -> bool {
    let Expression::CallExpression(call) = expr else {
        return false;
    };
    match helpers::unwrap_expr(&call.callee) {
        Expression::Identifier(id) => {
            let Some(symbol) = ctx.reference_symbol(id) else {
                return false;
            };
            if let AstKind::Function(f) = function.kind() {
                if f.id
                    .as_ref()
                    .is_some_and(|name| ctx.binding_symbol(name) == Some(symbol))
                {
                    return true;
                }
            }
            matches!(ctx.outer_parent(function).map(AstNode::kind),
                Some(AstKind::VariableDeclarator(d))
                    if matches!(&d.id, BindingPattern::BindingIdentifier(name)
                        if ctx.binding_symbol(name) == Some(symbol)))
        }
        Expression::StaticMemberExpression(m)
            if matches!(
                helpers::unwrap_expr(&m.object),
                Expression::ThisExpression(_)
            ) =>
        {
            let name = m.property.name.as_str();
            match ctx.parent_kind(function) {
                Some(AstKind::MethodDefinition(method)) => method.key.is_specific_static_name(name),
                Some(AstKind::PropertyDefinition(field)) => {
                    // `poll = async () => …`: an arrow field sees the class's `this`.
                    matches!(function.kind(), AstKind::ArrowFunctionExpression(_))
                        && field.key.is_specific_static_name(name)
                }
                _ => false,
            }
        }
        _ => false,
    }
}

/// `p.catch(…)` or `p.then(ok, fail)`: the chain handles its own rejection.
fn handles_rejection(expr: &Expression<'_>) -> bool {
    let Expression::CallExpression(call) = expr else {
        return false;
    };
    let Expression::StaticMemberExpression(m) = helpers::unwrap_expr(&call.callee) else {
        return false;
    };
    match m.property.name.as_str() {
        "catch" => true,
        "then" => call.arguments.len() >= 2,
        _ => false,
    }
}

/// Is the expression visibly a promise?
fn is_promise(ctx: &Ctx<'_>, expr: &Expression<'_>, at: &AstNode<'_>) -> bool {
    match expr {
        Expression::NewExpression(new) => ctx.is_global(&new.callee, "Promise"),
        Expression::CallExpression(call) => match helpers::unwrap_expr(&call.callee) {
            Expression::Identifier(id) => {
                (id.name == "fetch" && ctx.is_global_reference(id))
                    || ((util::is_local_async_function(ctx, id)
                        || returns_declared_promise(ctx, id))
                        && !catches_everything_fn(ctx, id))
            }
            Expression::StaticMemberExpression(m) => {
                let name = m.property.name.as_str();
                match name {
                    "then" | "finally" | "catch" => !is_cypress_chain(&m.object),
                    "toPromise" => true,
                    // `Promise.resolve` / `Promise.reject` settle at once, on purpose.
                    "all" | "allSettled" | "race" | "any" => ctx.is_global(&m.object, "Promise"),
                    _ => {
                        matches!(
                            helpers::unwrap_expr(&m.object),
                            Expression::ThisExpression(_)
                        ) && is_own_async_method(ctx, at, name)
                    }
                }
            }
            _ => false,
        },
        _ => false,
    }
}

/// A TypeScript function declared in this file with a `Promise<…>` return type.
fn returns_declared_promise(ctx: &Ctx<'_>, callee: &IdentifierReference<'_>) -> bool {
    let Some(symbol) = ctx.reference_symbol(callee) else {
        return false;
    };
    let return_type = match ctx.symbol_declaration(symbol).kind() {
        AstKind::Function(f) => f.return_type.as_ref(),
        AstKind::VariableDeclarator(d) if d.kind == VariableDeclarationKind::Const => {
            match d.init.as_ref().map(helpers::unwrap_expr) {
                Some(Expression::ArrowFunctionExpression(a)) => a.return_type.as_ref(),
                Some(Expression::FunctionExpression(f)) => f.return_type.as_ref(),
                _ => None,
            }
        }
        _ => None,
    };
    return_type.is_some_and(|t| is_promise_type(&t.type_annotation))
        && !ctx.references_of(symbol).any(|r| r.is_write())
}

fn is_promise_type(ty: &TSType<'_>) -> bool {
    matches!(ty, TSType::TSTypeReference(r)
        if matches!(&r.type_name, TSTypeName::IdentifierReference(id) if id.name == "Promise"))
}

/// `this.name(…)` inside a method of a class that declares `name` as an `async` method (or an
/// `async` arrow field, or a method declared to return a `Promise`).
fn is_own_async_method(ctx: &Ctx<'_>, at: &AstNode<'_>, name: &str) -> bool {
    // `this` belongs to the nearest non-arrow function: it must be a method of the class.
    let Some(function) = ctx
        .ancestors(at)
        .find(|n| matches!(n.kind(), AstKind::Function(_)))
    else {
        return false;
    };
    let Some(AstKind::MethodDefinition(_)) = ctx.parent_kind(function) else {
        return false;
    };
    let Some(class) = ctx.ancestors(function).find_map(|n| match n.kind() {
        AstKind::Class(c) => Some(c),
        _ => None,
    }) else {
        return false;
    };
    let mut found = false;
    for element in &class.body.body {
        match element {
            ClassElement::MethodDefinition(m) if m.key.is_specific_static_name(name) => {
                if m.value.body.is_none() {
                    // Overload signatures: look at the implementation only.
                    continue;
                }
                let f = &m.value;
                if !(f.r#async && !f.generator
                    || f.return_type
                        .as_ref()
                        .is_some_and(|t| is_promise_type(&t.type_annotation)))
                    || f.body
                        .as_deref()
                        .is_some_and(|b| catches_everything(ctx, b))
                {
                    return false;
                }
                found = true;
            }
            ClassElement::PropertyDefinition(p) if p.key.is_specific_static_name(name) => {
                match p.value.as_ref().map(helpers::unwrap_expr) {
                    Some(Expression::ArrowFunctionExpression(a))
                        if a.r#async && !catches_everything(ctx, &a.body) =>
                    {
                        found = true;
                    }
                    _ => return false,
                }
            }
            _ => {}
        }
    }
    found
}

/// `async function f() { try { … } catch { … } }`: every path ends in the `catch` block, which
/// does not throw, so the returned promise does not reject in practice. Setup statements may come
/// before the `try` (declarations, guards returning plain values) as long as nothing there awaits,
/// throws, or returns something that might be a promise.
fn catches_everything(ctx: &Ctx<'_>, body: &FunctionBody<'_>) -> bool {
    let Some((Statement::TryStatement(t), setup)) = body.statements.split_last() else {
        return false;
    };
    let Some(handler) = &t.handler else {
        return false;
    };
    // A `catch` block that rethrows (even after logging) lets the failure through.
    if !handler.body.body.iter().all(|s| cannot_reject(s, false)) {
        return false;
    }
    setup.is_empty()
        || (setup.iter().all(|s| cannot_reject(s, true))
            && !ctx
                .text(Span::new(body.span.start, t.span.start))
                .contains("await"))
}

/// The statement cannot make the function reject: it contains no `throw` (outside nested
/// functions) and, with `plain_returns`, every `return` in it hands back a plain value (`await` is
/// checked on the source text by the caller).
fn cannot_reject(stmt: &Statement<'_>, plain_returns: bool) -> bool {
    let all = |list: &[Statement<'_>]| list.iter().all(|s| cannot_reject(s, plain_returns));
    match stmt {
        Statement::ThrowStatement(_) => false,
        Statement::ReturnStatement(r) => {
            !plain_returns || r.argument.as_ref().is_none_or(is_plain_value)
        }
        Statement::BlockStatement(b) => all(&b.body),
        Statement::IfStatement(i) => {
            cannot_reject(&i.consequent, plain_returns)
                && i.alternate
                    .as_ref()
                    .is_none_or(|s| cannot_reject(s, plain_returns))
        }
        Statement::ForStatement(f) => cannot_reject(&f.body, plain_returns),
        Statement::ForInStatement(f) => cannot_reject(&f.body, plain_returns),
        Statement::ForOfStatement(f) => cannot_reject(&f.body, plain_returns),
        Statement::WhileStatement(w) => cannot_reject(&w.body, plain_returns),
        Statement::DoWhileStatement(w) => cannot_reject(&w.body, plain_returns),
        Statement::LabeledStatement(l) => cannot_reject(&l.body, plain_returns),
        Statement::WithStatement(w) => cannot_reject(&w.body, plain_returns),
        Statement::SwitchStatement(s) => s.cases.iter().all(|c| all(&c.consequent)),
        Statement::TryStatement(t) => {
            all(&t.block.body)
                && t.handler.as_ref().is_none_or(|h| all(&h.body.body))
                && t.finalizer.as_ref().is_none_or(|f| all(&f.body))
        }
        // Declarations, expressions and jumps; nested functions and classes do not run here.
        _ => true,
    }
}

/// A returned value that is visibly not a promise: a literal, an object or array literal, a
/// template, `undefined`, or a unary expression.
fn is_plain_value(expr: &Expression<'_>) -> bool {
    match helpers::unwrap_expr(expr) {
        Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BigIntLiteral(_)
        | Expression::StringLiteral(_)
        | Expression::TemplateLiteral(_)
        | Expression::ObjectExpression(_)
        | Expression::ArrayExpression(_)
        | Expression::UnaryExpression(_) => true,
        Expression::Identifier(id) => id.name == "undefined",
        _ => false,
    }
}

/// The local function called by `callee` is `async` and catches everything in its body (a
/// plain function declared to return a `Promise` can still hand back one that rejects).
fn catches_everything_fn(ctx: &Ctx<'_>, callee: &IdentifierReference<'_>) -> bool {
    let Some(symbol) = ctx.reference_symbol(callee) else {
        return false;
    };
    match ctx.symbol_declaration(symbol).kind() {
        AstKind::Function(f) => {
            f.r#async
                && f.body
                    .as_deref()
                    .is_some_and(|b| catches_everything(ctx, b))
        }
        AstKind::VariableDeclarator(d) => match d.init.as_ref().map(helpers::unwrap_expr) {
            Some(Expression::ArrowFunctionExpression(a)) => {
                a.r#async && catches_everything(ctx, &a.body)
            }
            Some(Expression::FunctionExpression(f)) => {
                f.r#async
                    && f.body
                        .as_deref()
                        .is_some_and(|b| catches_everything(ctx, b))
            }
            _ => false,
        },
        _ => false,
    }
}

/// `cy.get(…).then(…)`: Cypress command chains look like promises but are not.
fn is_cypress_chain(expr: &Expression<'_>) -> bool {
    let mut current = helpers::unwrap_expr(expr);
    loop {
        current = match current {
            Expression::CallExpression(call) => helpers::unwrap_expr(&call.callee),
            Expression::StaticMemberExpression(m) => helpers::unwrap_expr(&m.object),
            Expression::ComputedMemberExpression(m) => helpers::unwrap_expr(&m.object),
            Expression::Identifier(id) => return id.name == "cy" || id.name == "Cypress",
            _ => return false,
        };
    }
}
