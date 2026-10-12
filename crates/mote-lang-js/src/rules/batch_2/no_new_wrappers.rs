//! `js/no-new-wrappers`

use crate::prelude::*;

pub struct NoNewWrappers;

pub static NO_NEW_WRAPPERS: RuleMeta = RuleMeta {
    severity: Severity::Medium,
    tags: &["suspicious"],
    remediation_minutes: 2,
    description: r#"Flags `new String(…)`, `new Number(…)`, and `new Boolean(…)`.

**Why:** these create wrapper *objects*, not primitive values. They behave surprisingly:
`new Boolean(false)` is truthy, `new String('a') === 'a'` is `false`, and `typeof` reports
`'object'`. There is practically never a reason to want one.

**Fix:** use the literal, or call the function without `new` to convert a value:
`String(x)`, `Number(x)`, `Boolean(x)`. A local class or variable with one of these names is not
reported.

```js
const flag = new Boolean(false);   // flagged: truthy object
const count = new Number(input);   // flagged

const flag = false;                // compliant
const count = Number(input);       // compliant
```
"#,
    ..RuleMeta::new(
        "js/no-new-wrappers",
        "js",
        "Primitive wrapper objects should not be created with 'new'",
    )
};

impl JsRule for NoNewWrappers {
    fn meta(&self) -> &'static RuleMeta {
        &NO_NEW_WRAPPERS
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::NewExpression]
    }

    fn callee_names(&self) -> &'static [&'static str] {
        &["String", "Number", "Boolean"]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::NewExpression(new) = node.kind() else {
            return;
        };
        let Expression::Identifier(callee) = helpers::unwrap_expr(&new.callee) else {
            return;
        };
        let name = callee.name.as_str();
        if !matches!(name, "String" | "Number" | "Boolean") || !ctx.is_global_reference(callee) {
            return;
        }
        ctx.emit(
            Span::new(new.span.start, callee.span.end),
            format!(
                "Use a primitive value or '{name}(…)' without 'new' instead of a '{name}' object"
            ),
        );
    }
}
