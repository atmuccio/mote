//! `js/no-nested-component-definition`

use super::util;
use crate::prelude::*;

pub struct NoNestedComponentDefinition;

pub static NO_NESTED_COMPONENT_DEFINITION: RuleMeta = RuleMeta {
    tags: &["react", "performance"],
    remediation_minutes: 15,
    description: r#"Flags React components defined inside the body of another component: a capitalized
function that returns JSX, declared directly in a component and then rendered as `<Inner />` or
passed on as a component.

**Why:** each render of the outer component creates a brand-new `Inner` function. React compares
component types by identity, so it sees a different component every time: it unmounts the old
subtree and mounts a new one on every render. All state inside `Inner` (and its children) is reset,
inputs lose focus, effects re-run, and the whole subtree is rebuilt from scratch.

**Fix:** move the inner component to the top level of the module and pass it the values it needs as
props. If it only exists to share markup, call it as a plain function (`{renderRow(item)}`) or
inline the JSX.

```jsx
function List({ items }) {
  function Row({ item }) { return <li>{item.name}</li>; }  // flagged
  return <ul>{items.map((item) => <Row key={item.id} item={item} />)}</ul>;
}

function Row({ item }) { return <li>{item.name}</li>; }    // compliant: module level
function List({ items }) {
  return <ul>{items.map((item) => <Row key={item.id} item={item} />)}</ul>;
}
```

Only React/Preact files are checked. Inner functions that are only ever called directly
(`{Header()}`) are not flagged, because React never sees them as components, and neither are
components created inside hooks such as `useMemo` or `useCallback`.
"#,
    ..RuleMeta::new(
        "js/no-nested-component-definition",
        "js",
        "Components should not be defined inside other components",
    )
};

impl JsRule for NoNestedComponentDefinition {
    fn meta(&self) -> &'static RuleMeta {
        &NO_NESTED_COMPONENT_DEFINITION
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::Function, AstType::VariableDeclarator]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        util::has_jsx(ctx) && util::is_react_file(ctx)
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        // The inner definition: its name and whether it returns JSX.
        let (id, jsx) = match node.kind() {
            AstKind::Function(f) if f.is_declaration() => {
                let Some(id) = &f.id else {
                    return;
                };
                (id, function_returns_jsx(f))
            }
            AstKind::VariableDeclarator(d) => {
                let BindingPattern::BindingIdentifier(id) = &d.id else {
                    return;
                };
                let jsx = match d.init.as_ref().map(helpers::unwrap_expr) {
                    Some(Expression::ArrowFunctionExpression(a)) => arrow_returns_jsx(a),
                    Some(Expression::FunctionExpression(f)) => function_returns_jsx(f),
                    _ => return,
                };
                (id.as_ref(), jsx)
            }
            _ => return,
        };
        let name = id.name.as_str();
        if !jsx || !util::is_capitalized(name) {
            return;
        }
        // The nearest enclosing function must itself be a component.
        let Some(outer) = ctx.enclosing_function(node) else {
            return;
        };
        let Some(outer_name) = component_name(ctx, outer) else {
            return;
        };
        let Some(symbol) = ctx.binding_symbol(id) else {
            return;
        };
        if !used_as_component(ctx, symbol) {
            return;
        }
        ctx.emit(
            id.span,
            format!(
                "Move the component '{name}' out of '{outer_name}': it is re-created on every render, so React remounts it and loses its state"
            ),
        );
    }
}

/// A capitalized function that returns JSX, and its name: a function declaration or named
/// function expression, or a function stored in a variable (possibly wrapped in `memo(…)` or
/// `forwardRef(…)`).
fn component_name<'a>(ctx: &Ctx<'a>, function: &AstNode<'a>) -> Option<&'a str> {
    if !returns_jsx(function.kind()) {
        return None;
    }
    let name = match function.kind() {
        AstKind::Function(f) if f.id.is_some() => f.id.as_ref().map(|id| id.name.as_str()),
        AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => {
            let mut parent = ctx.outer_parent(function)?;
            if let AstKind::CallExpression(call) = parent.kind() {
                if !is_component_wrapper(&call.callee) {
                    return None;
                }
                parent = ctx.outer_parent(parent)?;
            }
            match parent.kind() {
                AstKind::VariableDeclarator(d) => match &d.id {
                    BindingPattern::BindingIdentifier(id) => Some(id.name.as_str()),
                    _ => None,
                },
                _ => None,
            }
        }
        _ => None,
    }?;
    util::is_capitalized(name).then_some(name)
}

/// `memo`, `forwardRef`, `React.memo`, `React.forwardRef`, `observer`.
fn is_component_wrapper(callee: &Expression<'_>) -> bool {
    let name = match helpers::unwrap_expr(callee) {
        Expression::Identifier(id) => id.name.as_str(),
        Expression::StaticMemberExpression(m) => m.property.name.as_str(),
        _ => return false,
    };
    matches!(name, "memo" | "forwardRef" | "observer")
}

/// Does the function return JSX (an arrow expression body, or a `return` of its own body)?
fn returns_jsx(kind: AstKind<'_>) -> bool {
    match kind {
        AstKind::ArrowFunctionExpression(a) => arrow_returns_jsx(a),
        AstKind::Function(f) => function_returns_jsx(f),
        _ => false,
    }
}

fn arrow_returns_jsx(arrow: &ArrowFunctionExpression<'_>) -> bool {
    if arrow.expression {
        arrow.body.statements.first().is_some_and(|s| match s {
            Statement::ExpressionStatement(e) => is_jsx(&e.expression),
            _ => false,
        })
    } else {
        body_returns_jsx(&arrow.body.statements)
    }
}

fn function_returns_jsx(function: &Function<'_>) -> bool {
    function
        .body
        .as_ref()
        .is_some_and(|b| body_returns_jsx(&b.statements))
}

fn body_returns_jsx(statements: &[Statement<'_>]) -> bool {
    let mut found = false;
    for_each_return(statements, &mut |r| {
        if r.argument.as_ref().is_some_and(is_jsx) {
            found = true;
        }
    });
    found
}

/// JSX, possibly as a branch of `cond ? <A /> : null` or `cond && <A />`.
fn is_jsx(expr: &Expression<'_>) -> bool {
    match helpers::unwrap_expr(expr) {
        Expression::JSXElement(_) | Expression::JSXFragment(_) => true,
        Expression::ConditionalExpression(c) => is_jsx(&c.consequent) || is_jsx(&c.alternate),
        Expression::LogicalExpression(l) => is_jsx(&l.right),
        _ => false,
    }
}

/// React sees the function as a component: it is rendered as `<Inner />` or passed on as a
/// value (`component={Inner}`). Functions that are only called directly are plain helpers.
fn used_as_component(ctx: &Ctx<'_>, symbol: SymbolId) -> bool {
    ctx.references_of(symbol).any(|reference| {
        let node = reference.node_id();
        match ctx.parent_kind(node) {
            Some(AstKind::CallExpression(call)) => {
                // `Inner(props)` as callee is a direct call; as an argument it is passed on.
                helpers::unwrap_expr(&call.callee).span() != ctx.node(node).span()
            }
            Some(AstKind::ExpressionStatement(_)) => false,
            _ => true,
        }
    })
}

/// Visit the `return` statements of a function body (not those of nested functions).
fn for_each_return<'a>(
    statements: &'a [Statement<'a>],
    f: &mut dyn FnMut(&'a ReturnStatement<'a>),
) {
    for stmt in statements {
        return_in_statement(stmt, f);
    }
}

fn return_in_statement<'a>(stmt: &'a Statement<'a>, f: &mut dyn FnMut(&'a ReturnStatement<'a>)) {
    match stmt {
        Statement::ReturnStatement(r) => f(r),
        Statement::BlockStatement(b) => for_each_return(&b.body, f),
        Statement::IfStatement(s) => {
            return_in_statement(&s.consequent, f);
            if let Some(alt) = &s.alternate {
                return_in_statement(alt, f);
            }
        }
        Statement::SwitchStatement(s) => {
            for case in &s.cases {
                for_each_return(&case.consequent, f);
            }
        }
        Statement::TryStatement(s) => {
            for_each_return(&s.block.body, f);
            if let Some(handler) = &s.handler {
                for_each_return(&handler.body.body, f);
            }
            if let Some(finalizer) = &s.finalizer {
                for_each_return(&finalizer.body, f);
            }
        }
        Statement::LabeledStatement(s) => return_in_statement(&s.body, f),
        Statement::ForStatement(s) => return_in_statement(&s.body, f),
        Statement::ForInStatement(s) => return_in_statement(&s.body, f),
        Statement::ForOfStatement(s) => return_in_statement(&s.body, f),
        Statement::WhileStatement(s) => return_in_statement(&s.body, f),
        Statement::DoWhileStatement(s) => return_in_statement(&s.body, f),
        _ => {}
    }
}
