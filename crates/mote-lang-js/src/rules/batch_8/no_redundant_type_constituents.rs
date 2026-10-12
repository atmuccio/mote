//! `js/no-redundant-type-constituents`

use crate::prelude::*;

pub struct NoRedundantTypeConstituents;

pub static NO_REDUNDANT_TYPE_CONSTITUENTS: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["typescript", "types", "redundant"],
    remediation_minutes: 2,
    description: r#"Flags members of TypeScript union (`A | B`) and intersection (`A & B`) types that add nothing:
the same member written twice, `any` or `unknown` in a union (they swallow every other member),
`never` in a union (it contributes no values), `any` or `unknown` in an intersection, and a literal
type next to its own base type (`'auto' | string`, `0 | number`, `true | boolean`).

**Why:** the type reads as if it listed distinct alternatives, but the compiler reduces it to
something simpler, so the listed alternatives are documentation that is not enforced. `'auto' |
string` looks like it singles out `'auto'`, yet it accepts every string; `User | any` turns off
type checking for every use while suggesting that values are users.

**Fix:** remove the absorbed member, or change the type to say what was meant (`'auto' | 'none'`,
`User | undefined`).

```ts
type Width = 'auto' | string;          // flagged: 'auto' is already a string
type Input = Config | any;             // flagged: any absorbs Config
type Id = string | number | string;    // flagged: string repeated

type Width = 'auto' | 'fill' | number; // compliant
type Input = Config | undefined;       // compliant
```

Only members written in the type itself are compared: type aliases, interfaces and types from
other files are out of reach. Not reported: the autocompletion idiom `'auto' | (string & {})`,
and `never` in an intersection (the compiler already reduces the whole type to `never`, which is
usually the point of writing it).
"#,
    ..RuleMeta::new(
        "js/no-redundant-type-constituents",
        "js",
        "Union and intersection types should not contain redundant members",
    )
};

impl JsRule for NoRedundantTypeConstituents {
    fn meta(&self) -> &'static RuleMeta {
        &NO_REDUNDANT_TYPE_CONSTITUENTS
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.is_typescript()
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::TSUnionType, AstType::TSIntersectionType]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let (types, union) = match node.kind() {
            AstKind::TSUnionType(u) => (&u.types, true),
            AstKind::TSIntersectionType(i) => (&i.types, false),
            _ => return,
        };
        let members: Vec<&TSType<'a>> = types.iter().map(unparen).collect();
        let separator = if union { "union" } else { "intersection" };

        // `any` / `unknown` / `never`.
        for member in &members {
            let keyword = match member {
                TSType::TSAnyKeyword(_) => "any",
                TSType::TSUnknownKeyword(_) => "unknown",
                TSType::TSNeverKeyword(_) if union => "never",
                _ => continue,
            };
            let message = match (keyword, union) {
                ("never", _) => "Remove 'never' from this union: it contributes no values".to_string(),
                ("unknown", false) => {
                    "Remove 'unknown' from this intersection: it does not narrow anything".to_string()
                }
                (_, true) => format!(
                    "Remove the other members or '{keyword}': '{keyword}' absorbs every member of this union"
                ),
                (_, false) => format!(
                    "Remove the other members or '{keyword}': '{keyword}' overrides every member of this intersection"
                ),
            };
            ctx.emit(member.span(), message);
            return;
        }

        // A literal next to its base type.
        for member in &members {
            let Some(base) = literal_base(member) else {
                continue;
            };
            if members.iter().any(|m| keyword_name(m) == Some(base)) {
                let literal = ctx.text(member.span());
                let message = if union {
                    format!(
                        "Remove '{literal}': the '{base}' member of this union already includes it"
                    )
                } else {
                    format!("Remove '{base}': the literal '{literal}' in this intersection is already a {base}")
                };
                ctx.emit(member.span(), message);
                return;
            }
        }

        // The same member twice (compared as written, ignoring whitespace).
        let mut seen: Vec<&str> = Vec::with_capacity(members.len());
        for member in &members {
            let text = ctx.text(member.span());
            if text.is_empty() {
                continue;
            }
            if seen.iter().any(|s| same_ignoring_whitespace(s, text)) {
                ctx.emit(
                    member.span(),
                    format!(
                        "Remove the repeated member '{}' of this {separator}",
                        ctx.text(member.span())
                    ),
                );
                return;
            }
            seen.push(text);
        }
    }
}

fn same_ignoring_whitespace(a: &str, b: &str) -> bool {
    a.chars()
        .filter(|c| !c.is_whitespace())
        .eq(b.chars().filter(|c| !c.is_whitespace()))
}

fn unparen<'b, 'a>(ty: &'b TSType<'a>) -> &'b TSType<'a> {
    match ty {
        TSType::TSParenthesizedType(p) => unparen(&p.type_annotation),
        _ => ty,
    }
}

/// The primitive keyword a literal type belongs to.
fn literal_base(ty: &TSType<'_>) -> Option<&'static str> {
    match ty {
        TSType::TSLiteralType(l) => match &l.literal {
            TSLiteral::BooleanLiteral(_) => Some("boolean"),
            TSLiteral::NumericLiteral(_) => Some("number"),
            TSLiteral::BigIntLiteral(_) => Some("bigint"),
            TSLiteral::StringLiteral(_) | TSLiteral::TemplateLiteral(_) => Some("string"),
            TSLiteral::UnaryExpression(u) => match &u.argument {
                Expression::NumericLiteral(_) => Some("number"),
                Expression::BigIntLiteral(_) => Some("bigint"),
                _ => None,
            },
        },
        TSType::TSTemplateLiteralType(_) => Some("string"),
        _ => None,
    }
}

fn keyword_name(ty: &TSType<'_>) -> Option<&'static str> {
    match ty {
        TSType::TSStringKeyword(_) => Some("string"),
        TSType::TSNumberKeyword(_) => Some("number"),
        TSType::TSBooleanKeyword(_) => Some("boolean"),
        TSType::TSBigIntKeyword(_) => Some("bigint"),
        _ => None,
    }
}
