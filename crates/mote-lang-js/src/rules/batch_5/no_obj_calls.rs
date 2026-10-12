//! `js/no-obj-calls`

use crate::prelude::*;

pub struct NoObjCalls;

pub static NO_OBJ_CALLS: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["suspicious"],
    remediation_minutes: 2,
    description: r#"Flags the built-in namespace objects `Math`, `JSON`, `Reflect`, `Atomics`, and `Intl` called as
functions (`Math()`) or constructors (`new JSON()`).

**Why:** these globals are plain objects that group related functions; they are neither functions
nor constructors. Calling them throws a `TypeError`. The code usually meant one of their members,
such as `Math.max(…)` or `JSON.parse(…)`.

**Fix:** call the intended member function.

```js
const biggest = Math(a, b);        // flagged: throws TypeError
const copy = new JSON(text);       // flagged

const biggest = Math.max(a, b);    // compliant
const copy = JSON.parse(text);     // compliant
```

Only the globals are checked: a local variable or import named `Math` or `Intl` is not reported.
"#,
    ..RuleMeta::new(
        "js/no-obj-calls",
        "js",
        "Built-in namespace objects should not be called",
    )
};

const NAMESPACES: &[&str] = &["Math", "JSON", "Reflect", "Atomics", "Intl"];

impl JsRule for NoObjCalls {
    fn meta(&self) -> &'static RuleMeta {
        &NO_OBJ_CALLS
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        let globals = ctx.scoping().root_unresolved_references();
        NAMESPACES.iter().any(|n| globals.contains_key(*n))
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CallExpression, AstType::NewExpression]
    }

    fn callee_names(&self) -> &'static [&'static str] {
        NAMESPACES
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let (callee, is_new) = match node.kind() {
            AstKind::CallExpression(call) => (&call.callee, false),
            AstKind::NewExpression(new) => (&new.callee, true),
            _ => return,
        };
        let Expression::Identifier(id) = helpers::unwrap_expr(callee) else {
            return;
        };
        let name = id.name.as_str();
        if !NAMESPACES.contains(&name) || !ctx.is_global_reference(id) {
            return;
        }
        let how = if is_new {
            "a constructor"
        } else {
            "a function"
        };
        ctx.emit(
            id.span,
            format!("Do not use '{name}' as {how}: it is a namespace object and this throws a TypeError; call one of its members"),
        );
    }
}
