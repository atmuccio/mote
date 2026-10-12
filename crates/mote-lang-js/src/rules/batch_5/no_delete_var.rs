//! `js/no-delete-var`

use crate::prelude::*;

pub struct NoDeleteVar;

pub static NO_DELETE_VAR: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["suspicious"],
    remediation_minutes: 2,
    description: r#"Flags `delete` applied to a plain variable name: `delete cache;`.

**Why:** `delete` removes *properties* from objects. On a variable declared with `var`, `let`,
`const`, or as a parameter it does nothing and evaluates to `false`; in strict mode and in ES
modules it is a syntax error. The author usually wanted to clear the variable or remove a property
of the object it holds.

**Fix:** assign `undefined` or `null` to release the value, or delete the property that was meant
(`delete cache[key]`).

```js
let session = load();
delete session;            // flagged: has no effect

session = null;            // compliant
delete registry[name];     // compliant: removes a property
```

Names that are declared nowhere in the file are not reported: they may be properties of the
global object (created by an assignment in another script), which `delete` does remove.
"#,
    ..RuleMeta::new(
        "js/no-delete-var",
        "js",
        "delete should not be applied to variables",
    )
};

impl JsRule for NoDeleteVar {
    fn meta(&self) -> &'static RuleMeta {
        &NO_DELETE_VAR
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::UnaryExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::UnaryExpression(unary) = node.kind() else {
            return;
        };
        if unary.operator != UnaryOperator::Delete {
            return;
        }
        let Expression::Identifier(id) = helpers::unwrap_expr(&unary.argument) else {
            return;
        };
        // An undeclared name may be a configurable global property, which `delete` does remove.
        if ctx.reference_symbol(id).is_none() {
            return;
        }
        let name = id.name.as_str();
        ctx.emit(
            unary.span,
            format!("Do not 'delete' the variable '{name}': delete only removes object properties; assign 'undefined' instead"),
        );
    }
}
