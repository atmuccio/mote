//! `js/no-self-state-update`

use crate::prelude::*;
use crate::rules::batch_6::util::{is_react_package, uses_global};

pub struct NoSelfStateUpdate;

pub static NO_SELF_STATE_UPDATE: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["react", "suspicious"],
    remediation_minutes: 5,
    description: r#"Flags a React state setter called with the state variable it belongs to:
`setItems(items)` for `const [items, setItems] = useState(…)`.

**Why:** React compares the new state with the current one and skips the update when they are the
same value (`Object.is`). So the call either does nothing, or — the usual story — it follows an
in-place change (`items.push(x); setItems(items);`) that React never sees: the component does not
re-render and the screen shows stale data. Called from an older closure, it can also quietly
reset the state to an outdated value.

**Fix:** pass a new value — `setItems([...items, x])`, `setUser({ ...user, name })` — or the
updater form `setItems((prev) => [...prev, x])`; remove the call if it has no purpose.

```jsx
const [items, setItems] = useState([]);
items.push(item);
setItems(items);                      // flagged: same array, no re-render
setItems([...items, item]);           // compliant
```

Only setters from `useState` imported from React (or Preact) are checked, and only when the
argument is the state variable itself (not a shadowing parameter of the same name).
"#,
    ..RuleMeta::new(
        "js/no-self-state-update",
        "js",
        "State setters should not be called with the current state value",
    )
};

impl JsRule for NoSelfStateUpdate {
    fn meta(&self) -> &'static RuleMeta {
        &NO_SELF_STATE_UPDATE
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CallExpression]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.source().contains("useState")
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::CallExpression(call) = node.kind() else {
            return;
        };
        let [Argument::Identifier(arg)] = call.arguments.as_slice() else {
            return;
        };
        let Expression::Identifier(callee) = helpers::unwrap_expr(&call.callee) else {
            return;
        };
        let (Some(setter), Some(value)) = (ctx.reference_symbol(callee), ctx.reference_symbol(arg))
        else {
            return;
        };
        let AstKind::VariableDeclarator(decl) = ctx.symbol_declaration(setter).kind() else {
            return;
        };
        let BindingPattern::ArrayPattern(pattern) = &decl.id else {
            return;
        };
        let (Some(Some(first)), Some(Some(second))) =
            (pattern.elements.first(), pattern.elements.get(1))
        else {
            return;
        };
        let (BindingPattern::BindingIdentifier(state), BindingPattern::BindingIdentifier(set)) =
            (first, second)
        else {
            return;
        };
        if ctx.binding_symbol(set) != Some(setter) || ctx.binding_symbol(state) != Some(value) {
            return;
        }
        if !decl
            .init
            .as_ref()
            .is_some_and(|init| is_use_state(ctx, init))
        {
            return;
        }
        let (name, set_name) = (state.name.as_str(), set.name.as_str());
        ctx.emit(
            call.span,
            format!(
                "'{set_name}({name})' passes the current state back: React skips the update, so an in-place change to '{name}' is never rendered; pass a new value instead"
            ),
        );
    }
}

/// `useState(…)` / `React.useState(…)` from a React-family package.
fn is_use_state(ctx: &Ctx<'_>, init: &Expression<'_>) -> bool {
    let Expression::CallExpression(call) = helpers::unwrap_expr(init) else {
        return false;
    };
    match helpers::unwrap_expr(&call.callee) {
        Expression::Identifier(id) => id.name == "useState" && imported_from_react(ctx, id),
        Expression::StaticMemberExpression(m) if m.property.name == "useState" => {
            match helpers::unwrap_expr(&m.object) {
                Expression::Identifier(object) => {
                    imported_from_react(ctx, object)
                        || (object.name == "React"
                            && ctx.is_global_reference(object)
                            && uses_global(ctx, "React"))
                }
                _ => false,
            }
        }
        _ => false,
    }
}

/// The identifier is bound by an import from a React-family package.
fn imported_from_react(ctx: &Ctx<'_>, id: &IdentifierReference<'_>) -> bool {
    let Some(symbol) = ctx.reference_symbol(id) else {
        return false;
    };
    let declaration = ctx.symbol_declaration(symbol);
    if !matches!(
        declaration.kind(),
        AstKind::ImportSpecifier(_)
            | AstKind::ImportDefaultSpecifier(_)
            | AstKind::ImportNamespaceSpecifier(_)
    ) {
        return false;
    }
    ctx.ancestors(declaration).any(|a| {
        matches!(a.kind(), AstKind::ImportDeclaration(import) if is_react_package(import.source.value.as_str()))
    })
}
