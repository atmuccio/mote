//! `js/default-param-last`

use crate::prelude::*;

pub struct DefaultParamLast;

pub static DEFAULT_PARAM_LAST: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["api-design", "confusing"],
    remediation_minutes: 5,
    description: r#"Flags a parameter with a default value that comes before a parameter without one.

**Why:** a default only applies when the argument is missing or `undefined`, and arguments are
matched by position. A caller that wants the default of an early parameter but has to pass a later
one must write `undefined` explicitly (`connect(undefined, options)`), which is easy to get wrong
and hides what the default was meant to do. Most of the time the defaulted parameter was simply
meant to be optional and belongs at the end.

**Fix:** move the parameters with defaults after the required ones, or group the optional values
in an options object.

```js
function connect(port = 8080, host) { … }        // flagged
connect(undefined, 'db.local');

function connect(host, port = 8080) { … }        // compliant
connect('db.local');
```

Not reported: parameters followed only by optional (`b?: T`), defaulted, rest or decorated
parameters (dependency injection fills decorated parameters, callers do not),
functions passed directly as arguments (their parameter order is set by the caller, as in
`reduce((acc = 0, x) => …)`), and Redux-style reducers (`(state = initial, action) => …`), whose
signature the store dictates.
"#,
    ..RuleMeta::new(
        "js/default-param-last",
        "js",
        "Parameters with defaults should come after the required ones",
    )
};

impl JsRule for DefaultParamLast {
    fn meta(&self) -> &'static RuleMeta {
        &DEFAULT_PARAM_LAST
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::FormalParameters]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::FormalParameters(params) = node.kind() else {
            return;
        };
        let items = params.items.as_slice();
        if items.len() < 2 {
            return;
        }
        // The last required parameter that callers pass: parameters filled by a
        // dependency-injection framework (`@IFoo foo`, `@Body() body`) do not count.
        let Some(last_required) = items
            .iter()
            .rposition(|p| !has_default(p) && !p.optional && p.decorators.is_empty())
        else {
            return;
        };
        let Some(first_default) = items[..last_required].iter().position(has_default) else {
            return;
        };
        let Some(function) = ctx.parent(node) else {
            return;
        };
        if !matches!(
            function.kind(),
            AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
        ) {
            return;
        }
        if matches!(
            ctx.outer_parent(function).map(AstNode::kind),
            Some(AstKind::CallExpression(_) | AstKind::NewExpression(_))
        ) {
            return;
        }
        let defaulted = &items[first_default];
        let required = &items[last_required];
        if is_reducer(defaulted, required) {
            return;
        }
        let name = binding_name(defaulted).unwrap_or("this destructured parameter");
        let later = binding_name(required).unwrap_or("the destructured parameter after it");
        ctx.emit(
            defaulted.span,
            format!(
                "Move '{name}' after '{later}': its default only applies when callers pass undefined explicitly"
            ),
        );
    }
}

fn has_default(param: &FormalParameter<'_>) -> bool {
    param.initializer.is_some() || matches!(param.pattern, BindingPattern::AssignmentPattern(_))
}

fn binding_name<'a>(param: &FormalParameter<'a>) -> Option<&'a str> {
    match &param.pattern {
        BindingPattern::BindingIdentifier(id) => Some(id.name.as_str()),
        BindingPattern::AssignmentPattern(a) => match &a.left {
            BindingPattern::BindingIdentifier(id) => Some(id.name.as_str()),
            _ => None,
        },
        _ => None,
    }
}

/// `(state = initialState, action)`.
fn is_reducer(defaulted: &FormalParameter<'_>, required: &FormalParameter<'_>) -> bool {
    binding_name(defaulted).is_some_and(|n| n == "state" || n.ends_with("State"))
        && binding_name(required).is_some_and(|n| n == "action" || n.ends_with("Action"))
}
