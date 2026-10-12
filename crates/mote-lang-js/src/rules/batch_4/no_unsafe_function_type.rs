//! `js/no-unsafe-function-type`

use crate::prelude::*;

pub struct NoUnsafeFunctionType;

pub static NO_UNSAFE_FUNCTION_TYPE: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["typescript", "types"],
    remediation_minutes: 5,
    description: r#"Flags the built-in `Function` type used in TypeScript type positions.

**Why:** `Function` accepts any function-like value — including classes — and calling a value of
that type is unchecked: any number of arguments of any type is accepted and the result is `any`.
It is `any` for functions: mistakes in how the callback is called or what it returns reach
runtime unnoticed.

**Fix:** write the actual signature, e.g. `(event: Event) => void`, or `(...args: never[]) =>
unknown` for "any function I will not call". Generic helpers can take a type parameter
constrained to a function type.

```ts
function onChange(listener: Function) {}            // flagged
function onChange(listener: (value: string) => void) {} // compliant
```

A local type or class that happens to be named `Function` is not reported.
"#,
    ..RuleMeta::new(
        "js/no-unsafe-function-type",
        "js",
        "The 'Function' type should not be used",
    )
};

impl JsRule for NoUnsafeFunctionType {
    fn meta(&self) -> &'static RuleMeta {
        &NO_UNSAFE_FUNCTION_TYPE
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::TSTypeReference]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.is_typescript()
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::TSTypeReference(reference) = node.kind() else {
            return;
        };
        let TSTypeName::IdentifierReference(id) = &reference.type_name else {
            return;
        };
        if id.name != "Function" || reference.type_arguments.is_some() {
            return;
        }
        // A local `type Function = …`, `interface Function`, class, or import shadows the global.
        if !ctx.is_global_reference(id) {
            return;
        }
        if ctx.in_ambient_context(node) {
            return;
        }
        ctx.emit(
            id.span,
            "Replace the 'Function' type with an explicit function signature",
        );
    }
}
