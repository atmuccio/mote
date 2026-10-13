//! `js/prefer-as-const`

use crate::prelude::*;

pub struct PreferAsConst;

pub static PREFER_AS_CONST: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["typescript", "redundant"],
    remediation_minutes: 1,
    description: r#"Flags a literal type that merely repeats the literal value next to it:
`let mode: 'dark' = 'dark'`, `'dark' as 'dark'`, `<'dark'>'dark'`, and class properties declared
the same way.

**Why:** the value is written twice, and the two copies must be kept in sync by hand — change one
and the code no longer compiles, or worse, a later edit updates only one of them. `as const` states
the intent ("keep the literal type") once.

**Fix:** write `'dark' as const` (or drop the annotation on a `const`, which already infers the
literal type).

```ts
let mode: 'dark' = 'dark';          // flagged
const size = 3 as 3;                // flagged
let strict: true = true;            // flagged (string, number, and boolean literals)

let mode = 'dark' as const;         // compliant
let mode: 'dark' | 'light' = 'dark'; // compliant: a wider type on purpose
```

**Fix available.** `mote fix` writes `as const` in place of the repeated literal type (`'dark'
as 'dark'` → `'dark' as const`), and for a variable moves it to the value (`let mode: 'dark' =
'dark'` → `let mode = 'dark' as const`). The type is the same literal type, and types do not
exist at run time. Class properties are reported but not rewritten.
"#,
    fixable: true,
    ..RuleMeta::new(
        "js/prefer-as-const",
        "js",
        "'as const' should be used instead of repeating a literal type",
    )
};

impl JsRule for PreferAsConst {
    fn meta(&self) -> &'static RuleMeta {
        &PREFER_AS_CONST
    }

    fn types(&self) -> &'static [AstType] {
        &[
            AstType::TSAsExpression,
            AstType::TSTypeAssertion,
            AstType::VariableDeclarator,
            AstType::PropertyDefinition,
        ]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.is_typescript()
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let (ty, value, is_cast) = match node.kind() {
            AstKind::TSAsExpression(e) => (&e.type_annotation, &e.expression, true),
            AstKind::TSTypeAssertion(e) => (&e.type_annotation, &e.expression, true),
            AstKind::VariableDeclarator(decl) => match (&decl.type_annotation, &decl.init) {
                (Some(a), Some(init)) => (&a.type_annotation, init, false),
                _ => return,
            },
            AstKind::PropertyDefinition(prop) => match (&prop.type_annotation, &prop.value) {
                (Some(a), Some(init)) => (&a.type_annotation, init, false),
                _ => return,
            },
            _ => return,
        };
        let TSType::TSLiteralType(lit) = ty else {
            return;
        };
        if !same_literal(&lit.literal, value) {
            return;
        }
        let message = if is_cast {
            "Use 'as const' instead of casting to the same literal type"
        } else {
            "Use 'as const' on the value instead of repeating it as the type"
        };
        let edits = if is_cast {
            Some(vec![Edit::replace(
                lit.span.start as usize,
                lit.span.end as usize,
                "const",
            )])
        } else if let AstKind::VariableDeclarator(decl) = node.kind() {
            declarator_edits(ctx, decl, value)
        } else {
            None
        };
        let emitted = ctx.emit(lit.span, message);
        if let Some(edits) = edits {
            emitted.fix("Use `as const`", edits);
        }
    }
}

/// `let x: 'a' = 'a'` → `let x = 'a' as const`: the annotation (from the end of the name) goes,
/// ` as const` follows the value. Only for a plain name with nothing but the annotation between
/// it and `=`.
fn declarator_edits(
    ctx: &Ctx<'_>,
    decl: &VariableDeclarator<'_>,
    value: &Expression<'_>,
) -> Option<Vec<Edit>> {
    let BindingPattern::BindingIdentifier(id) = &decl.id else {
        return None;
    };
    let ann = decl.type_annotation.as_ref()?;
    if decl.definite || ann.span.start < id.span.end || ctx.has_comments_in(decl.span) {
        return None;
    }
    let between = ctx.text(Span::new(id.span.end, ann.span.end));
    if !between.trim_start().starts_with(':') {
        return None;
    }
    Some(vec![
        Edit::delete(id.span.end as usize, ann.span.end as usize),
        Edit::insert(value.span().end as usize, " as const"),
    ])
}

fn same_literal(ty: &TSLiteral<'_>, value: &Expression<'_>) -> bool {
    match (ty, value) {
        (TSLiteral::StringLiteral(t), Expression::StringLiteral(v)) => t.value == v.value,
        (TSLiteral::NumericLiteral(t), Expression::NumericLiteral(v)) => t.value == v.value,
        (TSLiteral::BooleanLiteral(t), Expression::BooleanLiteral(v)) => t.value == v.value,
        _ => false,
    }
}
