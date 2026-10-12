//! `js/preserve-caught-error`

use crate::prelude::*;

pub struct PreserveCaughtError;

pub static PRESERVE_CAUGHT_ERROR: RuleMeta = RuleMeta {
    severity: Severity::Medium,
    tags: &["error-handling"],
    remediation_minutes: 5,
    params: &[ParamSpec {
        name: "quoted_message",
        default: ParamDefault::Bool(true),
        description: "Also report a new error that only quotes the caught error (`new Error('failed: ' + err.message)`) without passing it as `cause`.",
    }],
    description: r#"Flags a new built-in error (`Error`, `TypeError`, …) thrown from a `catch` block that does not
keep the caught error as its `cause`.

**Why:** the caught error carries the real cause — its message, its stack trace, and often a
chain of earlier causes. Throwing a fresh error that ignores it discards all of that, so the logs
only show the generic wrapper and the original failure is lost. Copying the caught error's
message into the new one (`new Error('failed: ' + err.message)`) keeps the words but still loses
the stack trace that says where the failure happened, and the cause chain behind it.

**Fix:** pass the caught error as the `cause` option (supported by all current engines and
Node.js 16.9+; TypeScript needs the `es2022` library). Error reporters and `console` print the
whole chain.

```js
try {
  config = JSON.parse(text);
} catch (err) {
  throw new Error('invalid configuration');                  // flagged: `err` is lost
}

try {
  config = JSON.parse(text);
} catch (err) {
  throw new Error(`invalid configuration: ${err.message}`);  // flagged: the stack is lost
}

try {
  config = JSON.parse(text);
} catch (err) {
  throw new Error('invalid configuration', { cause: err }); // compliant
}
```

Only `throw new …` directly in the `catch` block (not inside a nested function) is checked, and
only for the built-in error constructors, which all accept the `cause` option. An options object
counts when its `cause` mentions the caught error or names a variable declared in the `catch`
block; an options argument that is not an object literal (or that spreads another object) is
trusted. Not reported: a `catch` without a parameter, an `AggregateError` whose list of errors
mentions the caught error, and a new error whose message quotes the caught error's `stack`. With
the `quoted_message` parameter off, a new error that mentions the caught error anywhere in its
arguments is accepted, as it was before.
"#,
    ..RuleMeta::new(
        "js/preserve-caught-error",
        "js",
        "Errors thrown from a catch block should keep the caught error as their cause",
    )
};

const BUILT_IN_ERRORS: &[&str] = &[
    "Error",
    "TypeError",
    "RangeError",
    "SyntaxError",
    "ReferenceError",
    "EvalError",
    "URIError",
    "AggregateError",
];

impl JsRule for PreserveCaughtError {
    fn meta(&self) -> &'static RuleMeta {
        &PRESERVE_CAUGHT_ERROR
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::ThrowStatement]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::ThrowStatement(throw) = node.kind() else {
            return;
        };
        let Expression::NewExpression(new) = helpers::unwrap_expr(&throw.argument) else {
            return;
        };
        let Expression::Identifier(callee) = helpers::unwrap_expr(&new.callee) else {
            return;
        };
        let name = callee.name.as_str();
        if !BUILT_IN_ERRORS.contains(&name) || !ctx.is_global_reference(callee) {
            return;
        }
        if new
            .arguments
            .iter()
            .any(|a| matches!(a, Argument::SpreadElement(_)))
        {
            return;
        }
        let Some(catch) = enclosing_catch(ctx, node) else {
            return;
        };
        let Some(param) = &catch.param else {
            return;
        };
        let mut symbols = Vec::new();
        let mut unknown = false;
        helpers::for_each_binding(&param.pattern, &mut |id| match ctx.binding_symbol(id) {
            Some(symbol) => symbols.push(symbol),
            // Unknown binding: stay silent.
            None => unknown = true,
        });
        if unknown {
            return;
        }
        let message_args = if name == "AggregateError" { 2 } else { 1 };
        let caught = match &param.pattern {
            BindingPattern::BindingIdentifier(id) => Some(id.name.as_str()),
            _ => None,
        };
        let span = Span::new(new.span.start, new.callee.span().end);

        // `new Error(message, options)`: the options must carry the caught error as `cause`.
        if let Some(options) = new.arguments.get(message_args) {
            let Some(caught) = caught else {
                return;
            };
            let Some(Expression::ObjectExpression(object)) =
                options.as_expression().map(helpers::unwrap_expr)
            else {
                return;
            };
            // `new AggregateError([err, …], message, options)` keeps the errors in its list.
            if name == "AggregateError"
                && new
                    .arguments
                    .first()
                    .is_some_and(|a| !mentions_in(ctx, &symbols, a.span()).is_empty())
            {
                return;
            }
            if cause_keeps_error(ctx, node, object) != Some(false) {
                return;
            }
            ctx.emit(
                span,
                format!(
                    "Set the 'cause' option of this new '{name}' to the caught error '{caught}' so it is not lost"
                ),
            );
            return;
        }

        let mentions = mentions_in(ctx, &symbols, new.span);
        if mentions.is_empty() {
            let message = match caught {
                Some(caught) => format!(
                    "Pass '{{ cause: {caught} }}' to this new '{name}' so the caught error is not lost"
                ),
                None => format!(
                    "Pass the caught error as the cause of this new '{name}' so it is not lost"
                ),
            };
            ctx.emit(span, message);
            return;
        }
        // The new error quotes the caught one: only a plain binding can be passed on as it is.
        let Some(caught) = caught else {
            return;
        };
        // `new AggregateError([err, …], message)` keeps the errors in its list.
        if name == "AggregateError"
            && new
                .arguments
                .first()
                .is_some_and(|a| has_mention_in(ctx, &mentions, a.span()))
        {
            return;
        }
        // A message that quotes `err.stack` keeps the trace as text.
        if mentions.iter().any(|&m| reads_stack(ctx, m)) || !ctx.param_bool("quoted_message") {
            return;
        }
        ctx.emit(
            span,
            format!(
                "Pass '{{ cause: {caught} }}' to this new '{name}': quoting the caught error keeps its message but loses its stack trace and cause chain"
            ),
        );
    }
}

/// The references to any of `symbols` inside `span`.
fn mentions_in(ctx: &Ctx<'_>, symbols: &[SymbolId], span: Span) -> Vec<NodeId> {
    let mut found = Vec::new();
    for &symbol in symbols {
        for reference in ctx.references_of(symbol) {
            let at = ctx.node(reference.node_id()).span().start;
            if span.start <= at && at < span.end {
                found.push(reference.node_id());
            }
        }
    }
    found
}

/// Does any of the references lie inside `span`?
fn has_mention_in(ctx: &Ctx<'_>, mentions: &[NodeId], span: Span) -> bool {
    mentions.iter().any(|&m| {
        let at = ctx.node(m).span().start;
        span.start <= at && at < span.end
    })
}

/// `err.stack` (or `err?.stack`).
fn reads_stack(ctx: &Ctx<'_>, reference: NodeId) -> bool {
    match ctx.outer_parent(reference).map(AstNode::kind) {
        Some(AstKind::StaticMemberExpression(m)) => m.property.name == "stack",
        _ => false,
    }
}

/// Does the options object keep a caught error? `Some(true)`: its `cause` mentions the error of
/// the enclosing `catch` or of an outer `catch` of the same function (a nested cleanup that keeps
/// the first failure), or names a variable declared in one of those `catch` blocks (a wrapped or
/// normalized copy); `Some(false)`: there is no `cause`, or it is something else; `None`: the
/// object cannot be read (spread or computed keys).
fn cause_keeps_error(
    ctx: &Ctx<'_>,
    at: &AstNode<'_>,
    object: &ObjectExpression<'_>,
) -> Option<bool> {
    let mut cause = None;
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(p) = property else {
            return None;
        };
        if p.computed {
            return None;
        }
        if p.key.is_specific_static_name("cause") {
            cause = Some(&p.value);
        }
    }
    let Some(value) = cause else {
        return Some(false);
    };
    // Every `catch` around the throw, up to the enclosing function.
    let mut symbols = Vec::new();
    let mut bodies = Vec::new();
    for ancestor in ctx.ancestors(at) {
        match ancestor.kind() {
            AstKind::CatchClause(catch) => {
                bodies.push(catch.body.span);
                if let Some(param) = &catch.param {
                    helpers::for_each_binding(&param.pattern, &mut |id| {
                        symbols.extend(ctx.binding_symbol(id));
                    });
                }
            }
            kind if kind.is_function_like() || matches!(kind, AstKind::Class(_)) => break,
            _ => {}
        }
    }
    if !mentions_in(ctx, &symbols, value.span()).is_empty() {
        return Some(true);
    }
    if let Expression::Identifier(id) = helpers::unwrap_expr(value) {
        let Some(symbol) = ctx.reference_symbol(id) else {
            return Some(false);
        };
        let declared = ctx.symbol_span(symbol).start;
        return Some(
            bodies
                .iter()
                .any(|b| b.start <= declared && declared < b.end),
        );
    }
    Some(false)
}

/// The catch clause whose body contains `node` directly (not through a nested function, class, or
/// another `try` block's `finally`).
fn enclosing_catch<'a>(ctx: &Ctx<'a>, node: &AstNode<'a>) -> Option<&'a CatchClause<'a>> {
    for ancestor in ctx.ancestors(node) {
        match ancestor.kind() {
            AstKind::CatchClause(catch) => return Some(catch),
            AstKind::Function(_)
            | AstKind::ArrowFunctionExpression(_)
            | AstKind::Class(_)
            | AstKind::StaticBlock(_)
            | AstKind::PropertyDefinition(_)
            | AstKind::TryStatement(_) => return None,
            _ => {}
        }
    }
    None
}
