//! `js/no-object-constructor` (off by default; part of the `conventions` set)

use crate::prelude::*;

pub struct NoObjectConstructor;

pub static NO_OBJECT_CONSTRUCTOR: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Incubating,
    default_enabled: false,
    tags: &["conventions"],
    remediation_minutes: 1,
    description: r#"Flags `new Object()` and `Object()` called without arguments.

**Why:** both create an empty plain object, exactly like the literal `{}`, but longer and less
familiar; a reader may wonder whether a local `Object` or some special behavior is involved. The
literal form is also the one that can be written with its properties directly
(`{ id, name }`).

**Fix:** use an object literal.

```js
const cache = new Object();   // flagged
const cache = {};             // compliant
```

Not reported: calls with an argument (`Object(value)` converts a value to an object, which is a
different operation) and a local function or class named `Object`.

Off by default: part of the opt-in `conventions` set.
"#,
    ..RuleMeta::new(
        "js/no-object-constructor",
        "js",
        "Empty objects should be created with a literal, not the 'Object' constructor",
    )
};

impl JsRule for NoObjectConstructor {
    fn meta(&self) -> &'static RuleMeta {
        &NO_OBJECT_CONSTRUCTOR
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CallExpression, AstType::NewExpression]
    }

    fn callee_names(&self) -> &'static [&'static str] {
        &["Object"]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let (callee, args, span, is_new) = match node.kind() {
            AstKind::CallExpression(call) => (&call.callee, &call.arguments, call.span, false),
            AstKind::NewExpression(new) => (&new.callee, &new.arguments, new.span, true),
            _ => return,
        };
        if !args.is_empty() || !ctx.is_global(callee, "Object") {
            return;
        }
        let shown = if is_new { "new Object()" } else { "Object()" };
        ctx.emit(
            span,
            format!("Use the object literal '{{}}' instead of '{shown}'"),
        );
    }
}
