//! `js/incoming-value-overwritten`

use super::util;
use crate::prelude::*;

pub struct IncomingValueOverwritten;

pub static INCOMING_VALUE_OVERWRITTEN: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["suspicious", "unused"],
    remediation_minutes: 5,
    description: r#"Flags a parameter, a caught error, or a `for…of` / `for…in` loop variable that is assigned a new
value before its incoming value is ever read: `function scale(size) { size = 10; … }`.

**Why:** the value the caller passed (or the error that was caught, or the current element of the
loop) is thrown away unread. Either the assignment is a mistake — it was meant for another variable,
or it should only apply in some cases — or the parameter is not needed and callers are misled into
passing a value that has no effect.

**Fix:** assign to a new local variable, make the assignment conditional (`size ??= 10`), or remove
the parameter.

```js
function resize(image, width) {
  width = 800;                        // flagged: the width passed in is ignored
  return scale(image, width);
}

function resize(image, width = 800) { // compliant
  return scale(image, width);
}
```

Only plain assignments that are statements of the function, `catch`, or loop body itself, before
any statement that mentions the variable, are reported; an assignment that reads the old value
(`width = width || 800`) is not. Parameters of callbacks (functions passed as arguments, whose
signature the caller decides), of functions that use `arguments`, parameters read in another
parameter's default value, TypeScript parameter properties, names starting with `_`, and variables
used by nested functions are skipped.
"#,
    ..RuleMeta::new(
        "js/incoming-value-overwritten",
        "js",
        "Parameters and loop or catch variables should not be overwritten before they are read",
    )
};

#[derive(Clone, Copy)]
enum Binding {
    Parameter,
    CaughtError,
    LoopVariable,
}

impl JsRule for IncomingValueOverwritten {
    fn meta(&self) -> &'static RuleMeta {
        &INCOMING_VALUE_OVERWRITTEN
    }

    fn types(&self) -> &'static [AstType] {
        &[
            AstType::Function,
            AstType::ArrowFunctionExpression,
            AstType::CatchClause,
            AstType::ForOfStatement,
            AstType::ForInStatement,
        ]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        // `with` and direct `eval` can read any local by name.
        !ctx.file_contains(AstType::WithStatement) && !util::uses_global(ctx, "eval")
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        match node.kind() {
            AstKind::Function(f) => {
                let Some(body) = &f.body else {
                    return;
                };
                if !is_callback(ctx, node) {
                    check_parameters(ctx, &f.params, &body.statements, node.span());
                }
            }
            AstKind::ArrowFunctionExpression(a) => {
                if !a.expression && !is_callback(ctx, node) {
                    check_parameters(ctx, &a.params, &a.body.statements, node.span());
                }
            }
            AstKind::CatchClause(c) => {
                if let Some(BindingPattern::BindingIdentifier(id)) =
                    c.param.as_ref().map(|p| &p.pattern)
                {
                    check(ctx, id, &c.body.body, c.body.span, Binding::CaughtError);
                }
            }
            AstKind::ForOfStatement(s) => check_loop(ctx, &s.left, &s.body),
            AstKind::ForInStatement(s) => check_loop(ctx, &s.left, &s.body),
            _ => {}
        }
    }
}

/// A function passed as an argument: its caller decides what it receives, and a parameter may be
/// kept only to reach the next one.
fn is_callback(ctx: &Ctx<'_>, function: &AstNode<'_>) -> bool {
    matches!(
        ctx.outer_parent(function).map(AstNode::kind),
        Some(AstKind::CallExpression(_) | AstKind::NewExpression(_))
    )
}

fn check_parameters<'a>(
    ctx: &mut Ctx<'a>,
    params: &'a FormalParameters<'a>,
    body: &'a [Statement<'a>],
    function_span: Span,
) {
    if params.items.is_empty() || util::uses_arguments_in(ctx, function_span) {
        return;
    }
    let body_span = match (body.first(), body.last()) {
        (Some(first), Some(last)) => Span::new(first.span().start, last.span().end),
        _ => return,
    };
    for param in &params.items {
        if param.accessibility.is_some() || param.readonly || param.r#override {
            continue;
        }
        let id = match &param.pattern {
            BindingPattern::BindingIdentifier(id) => id,
            BindingPattern::AssignmentPattern(p) => match &p.left {
                BindingPattern::BindingIdentifier(id) => id,
                _ => continue,
            },
            _ => continue,
        };
        check(ctx, id, body, body_span, Binding::Parameter);
    }
}

fn check_loop<'a>(ctx: &mut Ctx<'a>, left: &'a ForStatementLeft<'a>, body: &'a Statement<'a>) {
    let ForStatementLeft::VariableDeclaration(decl) = left else {
        return;
    };
    if decl.kind != VariableDeclarationKind::Let {
        return;
    }
    let [declarator] = decl.declarations.as_slice() else {
        return;
    };
    let BindingPattern::BindingIdentifier(id) = &declarator.id else {
        return;
    };
    let Statement::BlockStatement(block) = body else {
        return;
    };
    check(ctx, id, &block.body, block.span, Binding::LoopVariable);
}

/// Report `name = value;` when it is the first statement of `body` that mentions the binding.
fn check<'a>(
    ctx: &mut Ctx<'a>,
    id: &BindingIdentifier<'a>,
    body: &'a [Statement<'a>],
    body_span: Span,
    binding: Binding,
) {
    let name = id.name.as_str();
    if name.starts_with('_') || body.is_empty() {
        return;
    }
    let Some(symbol) = ctx.binding_symbol(id) else {
        return;
    };
    if !ctx.scoping().symbol_redeclarations(symbol).is_empty() {
        return;
    }
    // The first mention decides; the cheap checks on it come before the checks of every
    // reference.
    let position = |r: &Reference| ctx.node(r.node_id()).span().start;
    let Some(first) = ctx.references_of(symbol).map(position).min() else {
        return;
    };
    let Some(statement) = body
        .iter()
        .find(|s| s.span().start <= first && first < s.span().end)
    else {
        return;
    };
    let Statement::ExpressionStatement(stmt) = statement else {
        return;
    };
    let Expression::AssignmentExpression(assign) = &stmt.expression else {
        return;
    };
    if assign.operator != AssignmentOperator::Assign {
        return;
    }
    let AssignmentTarget::AssignmentTargetIdentifier(target) = &assign.left else {
        return;
    };
    if ctx.reference_symbol(target) != Some(symbol) || target.span.start != first {
        return;
    }
    let own_scope = ctx.scoping().symbol_scope_id(symbol);
    for reference in ctx.references_of(symbol) {
        let at = ctx.node(reference.node_id()).span();
        // Read elsewhere (another parameter's default) or by a nested function: stay silent.
        if !util::within(at, body_span) || !same_function(ctx, reference.scope_id(), own_scope) {
            return;
        }
    }
    let right = assign.right.span();
    if ctx
        .references_of(symbol)
        .map(position)
        .any(|p| right.start <= p && p < right.end)
    {
        return;
    }
    let what = match binding {
        Binding::Parameter => format!("The value passed in for '{name}' is never read"),
        Binding::CaughtError => format!("The caught error '{name}' is never read"),
        Binding::LoopVariable => format!("The current element in '{name}' is never read"),
    };
    ctx.emit(
        util::first_line(ctx, assign.span),
        format!("{what}: it is overwritten here first; assign to another variable or remove it"),
    );
}

/// Do two scopes belong to the same function (no function or class boundary between the inner one
/// and the outer one)?
fn same_function(ctx: &Ctx<'_>, inner: ScopeId, outer: ScopeId) -> bool {
    let scoping = ctx.scoping();
    for scope in scoping.scope_ancestors(inner) {
        if scope == outer {
            return true;
        }
        let flags = scoping.scope_flags(scope);
        if flags.intersects(ScopeFlags::Function | ScopeFlags::Arrow | ScopeFlags::ClassStaticBlock)
            || matches!(
                ctx.node(scoping.get_node_id(scope)).kind(),
                AstKind::Class(_)
            )
        {
            return false;
        }
    }
    false
}
