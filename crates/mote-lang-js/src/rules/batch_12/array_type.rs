//! `js/array-type` (off by default; part of the `conventions` set)

use crate::prelude::*;

pub struct ArrayType;

pub static ARRAY_TYPE: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["conventions", "typescript"],
    remediation_minutes: 1,
    params: &[
        ParamSpec {
            name: "default",
            default: ParamDefault::Str("array-simple"),
            description: "Style for array types: `array` (always `T[]`), `generic` (always `Array<T>`) or `array-simple` (`T[]` when `T` is a simple name or keyword, `Array<T>` otherwise).",
        },
        ParamSpec {
            name: "readonly",
            default: ParamDefault::Str(""),
            description: "Style for read-only array types (`readonly T[]` / `ReadonlyArray<T>`); empty = same as `default`.",
        },
    ],
    description: r#"Flags array types written in the other of the two equivalent forms, `T[]` and `Array<T>`
(and `readonly T[]` / `ReadonlyArray<T>`). With the default `array-simple` style, a simple
element type (a keyword, a type name without type arguments, `this`, or an array of those) uses
the short form and anything else uses the generic form.

**Why:** both forms mean the same, and mixing them makes the same type look different from one
line to the next. The short form reads best for simple elements, while for unions, functions or
object types the suffix `[]` is easy to misread (`string | number[]` is not
`(string | number)[]`) and the generic form makes the element type explicit.

**Fix:** rewrite the type in the configured form.

```ts
let names: Array<string>;          // flagged: string[]
let ids: (string | number)[];      // flagged: Array<string | number>
let rows: readonly Row[];          // compliant
let handlers: Array<() => void>;   // compliant
```

Not reported: `Array` or `ReadonlyArray` names declared in the file itself, and references with
no type argument or more than one. Only TypeScript files are checked.

Off by default: part of the opt-in `conventions` set.
"#,
    ..RuleMeta::new(
        "js/array-type",
        "js",
        "Array types should use one consistent syntax",
    )
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Style {
    Array,
    Generic,
    ArraySimple,
}

fn parse_style(s: &str) -> Option<Style> {
    match s.trim() {
        "array" => Some(Style::Array),
        "generic" => Some(Style::Generic),
        "array-simple" => Some(Style::ArraySimple),
        _ => None,
    }
}

fn style(ctx: &Ctx<'_>, readonly: bool) -> Style {
    let default = parse_style(ctx.param_str("default")).unwrap_or(Style::ArraySimple);
    if readonly {
        parse_style(ctx.param_str("readonly")).unwrap_or(default)
    } else {
        default
    }
}

fn is_simple(ty: &TSType<'_>) -> bool {
    match ty {
        TSType::TSAnyKeyword(_)
        | TSType::TSBigIntKeyword(_)
        | TSType::TSBooleanKeyword(_)
        | TSType::TSNeverKeyword(_)
        | TSType::TSNullKeyword(_)
        | TSType::TSNumberKeyword(_)
        | TSType::TSObjectKeyword(_)
        | TSType::TSStringKeyword(_)
        | TSType::TSSymbolKeyword(_)
        | TSType::TSUndefinedKeyword(_)
        | TSType::TSUnknownKeyword(_)
        | TSType::TSVoidKeyword(_)
        | TSType::TSThisType(_) => true,
        TSType::TSTypeReference(r) => r.type_arguments.is_none(),
        TSType::TSArrayType(a) => is_simple(&a.element_type),
        TSType::TSParenthesizedType(p) => is_simple(&p.type_annotation),
        _ => false,
    }
}

impl JsRule for ArrayType {
    fn meta(&self) -> &'static RuleMeta {
        &ARRAY_TYPE
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::TSTypeReference, AstType::TSArrayType]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.is_typescript()
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        match node.kind() {
            AstKind::TSTypeReference(r) => {
                let TSTypeName::IdentifierReference(id) = &r.type_name else {
                    return;
                };
                let readonly = match id.name.as_str() {
                    "Array" => false,
                    "ReadonlyArray" => true,
                    _ => return,
                };
                let Some(args) = &r.type_arguments else {
                    return;
                };
                let [element] = args.params.as_slice() else {
                    return;
                };
                if ctx.reference_symbol(id).is_some() {
                    return; // a local type named Array
                }
                let flag = match style(ctx, readonly) {
                    Style::Generic => false,
                    Style::Array => true,
                    Style::ArraySimple => is_simple(element),
                };
                if flag {
                    let (short, long) = if readonly {
                        ("readonly T[]", "ReadonlyArray<T>")
                    } else {
                        ("T[]", "Array<T>")
                    };
                    ctx.emit(r.span, format!("Use '{short}' instead of '{long}'"));
                }
            }
            AstKind::TSArrayType(a) => {
                // `(A | B)[][]`: the outer array is reported too, so the inner one is left out.
                // (Tested first: `is_simple` looks through every level of `T[][]…`.)
                if matches!(ctx.parent_kind(node), Some(AstKind::TSArrayType(_)))
                    && style(ctx, false) == Style::ArraySimple
                {
                    return;
                }
                let readonly_parent = match ctx.parent_kind(node) {
                    Some(AstKind::TSTypeOperator(op))
                        if op.operator == TSTypeOperatorOperator::Readonly =>
                    {
                        Some(op.span)
                    }
                    _ => None,
                };
                let flag = match style(ctx, readonly_parent.is_some()) {
                    Style::Array => false,
                    Style::Generic => true,
                    Style::ArraySimple => !is_simple(&a.element_type),
                };
                if !flag {
                    return;
                }
                let (short, long, span) = match readonly_parent {
                    Some(span) => ("readonly T[]", "ReadonlyArray<T>", span),
                    None => ("T[]", "Array<T>", a.span),
                };
                ctx.emit(span, format!("Use '{long}' instead of '{short}'"));
            }
            _ => {}
        }
    }
}
