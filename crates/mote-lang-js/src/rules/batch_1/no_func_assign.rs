//! `js/no-func-assign`

use crate::prelude::*;

pub struct NoFuncAssign;

pub static NO_FUNC_ASSIGN: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["suspicious"],
    remediation_minutes: 5,
    description: r#"Flags assignments to the name of a function declaration (`function f() {}` … `f = …`),
including the name of a named function expression inside its own body.

**Why:** overwriting a declared function replaces it for every caller, which is rarely intended
and very hard to follow; usually the assignment was meant for a different variable. Assigning to
the name of a named function expression from inside it silently does nothing in sloppy mode and
throws in strict mode.

**Fix:** assign to a separate variable. If the function really needs to be swappable, declare it
with `let` (`let handler = function () {}`) so the intent is explicit.

```js
function handler() {}
handler = () => {};         // flagged

let handler = function () {};
handler = () => {};         // compliant: an ordinary variable
```
"#,
    ..RuleMeta::new(
        "js/no-func-assign",
        "js",
        "Function declarations should not be reassigned",
    )
};

impl JsRule for NoFuncAssign {
    fn meta(&self) -> &'static RuleMeta {
        &NO_FUNC_ASSIGN
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::Function]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::Function(function) = node.kind() else {
            return;
        };
        if function.body.is_none() || function.declare {
            return;
        }
        let Some(id) = &function.id else {
            return;
        };
        let Some(symbol) = ctx.binding_symbol(id) else {
            return;
        };
        // `var f; function f() {}`, overloads, and namespace merges are other declarations.
        if !ctx.scoping().symbol_redeclarations(symbol).is_empty() {
            return;
        }
        let writes: Vec<Span> = ctx
            .references_of(symbol)
            .filter(|r| r.is_write())
            .map(|r| ctx.node(r.node_id()).span())
            .collect();
        for span in writes {
            ctx.emit(
                span,
                format!(
                    "Do not reassign the function '{}'; use a separate variable",
                    id.name
                ),
            );
        }
    }
}
