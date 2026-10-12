//! `js/no-param-reassign`

use crate::prelude::*;

pub struct NoParamReassign;

pub static NO_PARAM_REASSIGN: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["confusing"],
    remediation_minutes: 5,
    description: r#"Flags assignments to a function parameter: `param = …`, `param += …`, `param++`,
destructuring assignments, and `for (param of …)`.

**Why:** after a reassignment, the parameter no longer holds what the caller passed, so readers
(and debuggers) looking at the name can be misled. In non-strict code, reassigning a parameter
also changes `arguments[i]`.

**Fix:** copy the value into a local variable and work with that, or use a default parameter
value. Changing a property of the object a parameter points to (`options.x = 1`) is not
reported.

```js
function greet(name) {
  name = name || 'guest';          // flagged
  return `Hello ${name}`;
}

function greet(name = 'guest') {   // compliant
  return `Hello ${name}`;
}
```
"#,
    // Stylistic and noisy on real code: opt-in (`extends: all` or enable explicitly).
    default_enabled: false,
    ..RuleMeta::new(
        "js/no-param-reassign",
        "js",
        "Function parameters should not be reassigned",
    )
};

impl JsRule for NoParamReassign {
    fn meta(&self) -> &'static RuleMeta {
        &NO_PARAM_REASSIGN
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::FormalParameter, AstType::FormalParameterRest]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let pattern = match node.kind() {
            AstKind::FormalParameter(p) => &p.pattern,
            AstKind::FormalParameterRest(r) => &r.rest.argument,
            _ => return,
        };
        let mut hits: Vec<(Span, &'a str)> = Vec::new();
        helpers::for_each_binding(pattern, &mut |id| {
            let Some(symbol) = id.symbol_id.get() else {
                return;
            };
            for reference in ctx.references_of(symbol) {
                if reference.flags().is_write() {
                    let span = ctx.node(reference.node_id()).span();
                    hits.push((span, id.name.as_str()));
                }
            }
        });
        for (span, name) in hits {
            ctx.emit(
                span,
                format!("Assign to a local variable instead of reassigning parameter '{name}'"),
            );
        }
    }
}
