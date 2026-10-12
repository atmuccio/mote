//! `js/prefer-rest-params`

use crate::prelude::*;

pub struct PreferRestParams;

pub static PREFER_REST_PARAMS: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["es2015", "convention"],
    remediation_minutes: 5,
    description: r#"Flags uses of the implicit `arguments` object inside functions.

**Why:** `arguments` is not a real array (no `map`, `slice`, or `forEach` without borrowing them
from `Array.prototype`), it hides from the signature which values the function accepts, and in
sloppy mode it is linked to the named parameters, so assigning one silently changes the other.
Arrow functions do not even have their own `arguments`.

**Fix:** declare a rest parameter, `function log(...parts)`, which is a real array and shows up in
the signature.

```js
function sum() {
  return Array.prototype.reduce.call(arguments, (a, b) => a + b, 0); // flagged
}

function sum(...values) {                                               // compliant
  return values.reduce((a, b) => a + b, 0);
}
```

`arguments.length` and `arguments.callee` on their own are not reported.
"#,
    ..RuleMeta::new(
        "js/prefer-rest-params",
        "js",
        "Rest parameters should be used instead of 'arguments'",
    )
};

impl JsRule for PreferRestParams {
    fn meta(&self) -> &'static RuleMeta {
        &PREFER_REST_PARAMS
    }

    fn run_once<'a>(&self, ctx: &mut Ctx<'a>) {
        let scoping = ctx.scoping();
        let Some(ids) = scoping.root_unresolved_references().get("arguments") else {
            return;
        };
        let mut spans: Vec<Span> = Vec::new();
        for &id in ids {
            let node = ctx.node(scoping.get_reference(id).node_id());
            let AstKind::IdentifierReference(ident) = node.kind() else {
                continue;
            };
            // `arguments.length` / `arguments.callee` do not index into the arguments.
            if let Some(AstKind::StaticMemberExpression(m)) = ctx.parent_kind(node) {
                if m.object.span() == ident.span {
                    continue;
                }
            }
            if !in_own_function(ctx, node) || ctx.in_ambient_context(node) {
                continue;
            }
            spans.push(ident.span);
        }
        spans.sort_unstable_by_key(|s| s.start);
        for span in spans {
            ctx.emit(
                span,
                "Use a rest parameter ('...args') instead of the 'arguments' object",
            );
        }
    }
}

/// Is the reference inside a non-arrow function (arrow functions look through to the outer
/// function's `arguments`)? Class field initializers and static blocks have no `arguments`.
fn in_own_function(ctx: &Ctx<'_>, node: &AstNode<'_>) -> bool {
    for ancestor in ctx.ancestors(node) {
        match ancestor.kind() {
            AstKind::ArrowFunctionExpression(_) => {}
            AstKind::Function(_) => return true,
            AstKind::StaticBlock(_) | AstKind::PropertyDefinition(_) | AstKind::Program(_) => {
                return false
            }
            _ => {}
        }
    }
    false
}
