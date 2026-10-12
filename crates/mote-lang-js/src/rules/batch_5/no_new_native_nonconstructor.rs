//! `js/no-new-native-nonconstructor`

use crate::prelude::*;

pub struct NoNewNativeNonconstructor;

pub static NO_NEW_NATIVE_NONCONSTRUCTOR: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["suspicious"],
    remediation_minutes: 2,
    description: r#"Flags `new Symbol(…)` and `new BigInt(…)`.

**Why:** `Symbol` and `BigInt` are functions that create primitive values, but they are not
constructors: using them with `new` throws a `TypeError`. The habit comes from `new String(…)` and
`new Number(…)`, which do work (and create wrapper objects nobody wants).

**Fix:** call them without `new`.

```js
const id = new Symbol('id');     // flagged: throws TypeError
const big = new BigInt(10);      // flagged

const id = Symbol('id');         // compliant
const big = BigInt(10);          // compliant
```

Only the globals are checked: a local class or import named `Symbol` is not reported.
"#,
    ..RuleMeta::new(
        "js/no-new-native-nonconstructor",
        "js",
        "Symbol and BigInt should not be used with new",
    )
};

impl JsRule for NoNewNativeNonconstructor {
    fn meta(&self) -> &'static RuleMeta {
        &NO_NEW_NATIVE_NONCONSTRUCTOR
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        let globals = ctx.scoping().root_unresolved_references();
        globals.contains_key("Symbol") || globals.contains_key("BigInt")
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::NewExpression]
    }

    fn callee_names(&self) -> &'static [&'static str] {
        &["Symbol", "BigInt"]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::NewExpression(new) = node.kind() else {
            return;
        };
        let Expression::Identifier(id) = helpers::unwrap_expr(&new.callee) else {
            return;
        };
        let name = id.name.as_str();
        if !matches!(name, "Symbol" | "BigInt") || !ctx.is_global_reference(id) {
            return;
        }
        let span = Span::new(new.span.start, id.span.end);
        ctx.emit(
            span,
            format!("Call '{name}(…)' without 'new': it is not a constructor and this throws a TypeError"),
        );
    }
}
