//! `js/no-set-state-in-render`

use super::util;
use crate::prelude::*;

pub struct NoSetStateInRender;

pub static NO_SET_STATE_IN_RENDER: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["react"],
    remediation_minutes: 10,
    description: r#"Flags a React state setter called unconditionally while a component renders: a statement
`setCount(…)` directly in the body of the component that declared `[count, setCount] =
useState(…)` (or a `useReducer` dispatch), or `this.setState(…)` directly in a class component's
`render()` method.

**Why:** setting state schedules another render. When the call runs on every render, each render
triggers the next one: React stops with a "Too many re-renders" error (function components) or
loops until the stack overflows (class components).

**Fix:** set state in an event handler or an effect, or compute the value during render instead of
storing it. To adjust state when a prop changes, guard the call with a condition that becomes
false after the update.

```jsx
function Counter({ start }) {
  const [count, setCount] = useState(0);
  setCount(start);                          // flagged: renders forever
  return <span>{count}</span>;
}

function Counter({ start }) {
  const [count, setCount] = useState(start); // compliant: initial state
  return <button onClick={() => setCount(count + 1)}>{count}</button>;
}
```

Only calls that certainly run on every render are reported: a statement directly in the component
body (not inside `if`, loops, `&&`, callbacks, or effects), with a setter that the same component
got from `useState` / `useReducer` imported from React or Preact (or called as `React.useState`).
"#,
    ..RuleMeta::new(
        "js/no-set-state-in-render",
        "js",
        "State setters should not be called unconditionally during render",
    )
};

impl JsRule for NoSetStateInRender {
    fn meta(&self) -> &'static RuleMeta {
        &NO_SET_STATE_IN_RENDER
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::ExpressionStatement]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        util::is_react_file(ctx)
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::ExpressionStatement(stmt) = node.kind() else {
            return;
        };
        let Expression::CallExpression(call) = helpers::unwrap_expr(&stmt.expression) else {
            return;
        };
        let Some(body) = ctx.parent(node) else {
            return;
        };
        if !matches!(body.kind(), AstKind::FunctionBody(_)) {
            return;
        }
        let Some(function) = ctx.parent(body) else {
            return;
        };
        match function.kind() {
            AstKind::Function(_) => {}
            AstKind::ArrowFunctionExpression(a) if !a.expression => {}
            _ => return,
        }
        match helpers::unwrap_expr(&call.callee) {
            Expression::Identifier(id) => {
                if is_own_state_setter(ctx, id, function) {
                    ctx.emit(
                        call.span,
                        format!(
                            "Do not call '{}' while rendering: each call schedules another render; set state in an event handler or an effect",
                            id.name
                        ),
                    );
                }
            }
            Expression::StaticMemberExpression(m)
                if m.property.name == "setState"
                    && matches!(
                        helpers::unwrap_expr(&m.object),
                        Expression::ThisExpression(_)
                    )
                    && is_class_render(ctx, function) =>
            {
                ctx.emit(
                        call.span,
                        "Do not call 'this.setState' in render(): each call schedules another render; set state in an event handler or a lifecycle method",
                    );
            }
            _ => {}
        }
    }
}

/// Is `callee` the setter (second element) of `const [value, setValue] = useState(…)` declared
/// directly in `function`, with `useState` / `useReducer` coming from React?
fn is_own_state_setter(
    ctx: &Ctx<'_>,
    callee: &IdentifierReference<'_>,
    function: &AstNode<'_>,
) -> bool {
    let Some(symbol) = ctx.reference_symbol(callee) else {
        return false;
    };
    if ctx.references_of(symbol).any(|r| r.is_write()) {
        return false;
    }
    let decl = ctx.symbol_declaration(symbol);
    let AstKind::VariableDeclarator(d) = decl.kind() else {
        return false;
    };
    let BindingPattern::ArrayPattern(pattern) = &d.id else {
        return false;
    };
    let is_second = matches!(pattern.elements.get(1), Some(Some(BindingPattern::BindingIdentifier(b)))
        if b.span == ctx.symbol_span(symbol));
    if !is_second {
        return false;
    }
    let Some(Expression::CallExpression(init)) = d.init.as_ref().map(helpers::unwrap_expr) else {
        return false;
    };
    if !is_react_state_hook(ctx, &init.callee) {
        return false;
    }
    // The state must belong to this component: the declaration sits directly in its body.
    ctx.ancestors(decl)
        .find(|n| {
            matches!(
                n.kind(),
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
            )
        })
        .is_some_and(|f| f.id() == function.id())
}

/// `useState` / `useReducer` imported from a React-family package, or `React.useState(…)`.
fn is_react_state_hook(ctx: &Ctx<'_>, callee: &Expression<'_>) -> bool {
    match helpers::unwrap_expr(callee) {
        Expression::Identifier(id) => {
            if !matches!(id.name.as_str(), "useState" | "useReducer") {
                return false;
            }
            let Some(symbol) = ctx.reference_symbol(id) else {
                return false;
            };
            let decl = ctx.symbol_declaration(symbol);
            if !matches!(decl.kind(), AstKind::ImportSpecifier(_)) {
                return false;
            }
            ctx.ancestors(decl).any(|n| {
                matches!(n.kind(), AstKind::ImportDeclaration(import)
                    if util::is_react_package(import.source.value.as_str()))
            })
        }
        Expression::StaticMemberExpression(m) => {
            matches!(m.property.name.as_str(), "useState" | "useReducer")
                && matches!(helpers::unwrap_expr(&m.object), Expression::Identifier(o)
                    if o.name == "React")
        }
        _ => false,
    }
}

/// Is `function` the `render()` method of a class that extends something?
fn is_class_render(ctx: &Ctx<'_>, function: &AstNode<'_>) -> bool {
    let Some(AstKind::MethodDefinition(m)) = ctx.parent_kind(function) else {
        return false;
    };
    if m.r#static
        || m.kind != MethodDefinitionKind::Method
        || !m.key.is_specific_static_name("render")
    {
        return false;
    }
    ctx.ancestors(function)
        .find_map(|n| match n.kind() {
            AstKind::Class(c) => Some(c.super_class.is_some()),
            _ => None,
        })
        .unwrap_or(false)
}
