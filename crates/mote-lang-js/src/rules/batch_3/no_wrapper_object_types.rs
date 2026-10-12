//! `js/no-wrapper-object-types`

use crate::prelude::*;

pub struct NoWrapperObjectTypes;

pub static NO_WRAPPER_OBJECT_TYPES: RuleMeta = RuleMeta {
    severity: Severity::Medium,
    tags: &["typescript"],
    remediation_minutes: 2,
    description: r#"Flags the built-in wrapper object types `String`, `Number`, `Boolean`, `BigInt`,
`Symbol`, and `Object` used as types.

**Why:** `String`, `Number`, … describe the *wrapper objects* created by `new String('x')`, not the
primitive values code actually passes around. They accept primitives (so mistakes go unnoticed) but
a value typed `Number` cannot be used in arithmetic, and wrapper objects compare by identity
(`new String('a') !== 'a'`). `Object` accepts every value except `null`/`undefined`, primitives
included, which is rarely the intent.

**Fix:** use the primitive types `string`, `number`, `boolean`, `bigint`, `symbol`; use `object`
for "any non-primitive" or `unknown` for "any value".

Only the global names are flagged: a locally declared or imported type with the same name is left
alone.

```ts
function greet(name: String): Boolean {}   // flagged twice
let options: Object = {};                  // flagged

function greet(name: string): boolean {}   // compliant
let options: object = {};                  // compliant
```
"#,
    ..RuleMeta::new(
        "js/no-wrapper-object-types",
        "js",
        "Primitive types should be used instead of wrapper object types",
    )
};

impl JsRule for NoWrapperObjectTypes {
    fn meta(&self) -> &'static RuleMeta {
        &NO_WRAPPER_OBJECT_TYPES
    }

    fn types(&self) -> &'static [AstType] {
        &[
            AstType::TSTypeReference,
            AstType::TSClassImplements,
            AstType::TSInterfaceHeritage,
        ]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.is_typescript()
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let id = match node.kind() {
            AstKind::TSTypeReference(r) if r.type_arguments.is_none() => match &r.type_name {
                TSTypeName::IdentifierReference(id) => id,
                _ => return,
            },
            AstKind::TSClassImplements(c) if c.type_arguments.is_none() => match &c.expression {
                TSTypeName::IdentifierReference(id) => id,
                _ => return,
            },
            AstKind::TSInterfaceHeritage(h) if h.type_arguments.is_none() => match &h.expression {
                Expression::Identifier(id) => id,
                _ => return,
            },
            _ => return,
        };
        let Some(replacement) = replacement(id.name.as_str()) else {
            return;
        };
        if !ctx.is_global_reference(id) {
            return;
        }
        ctx.emit(
            id.span,
            format!(
                "Use {replacement} instead of the wrapper type '{}'",
                id.name
            ),
        );
    }
}

fn replacement(name: &str) -> Option<&'static str> {
    Some(match name {
        "String" => "'string'",
        "Number" => "'number'",
        "Boolean" => "'boolean'",
        "BigInt" => "'bigint'",
        "Symbol" => "'symbol'",
        "Object" => "'object' or 'unknown'",
        _ => return None,
    })
}
