//! `js/no-unnecessary-type-constraint`

use crate::prelude::*;

pub struct NoUnnecessaryTypeConstraint;

pub static NO_UNNECESSARY_TYPE_CONSTRAINT: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["typescript", "redundant"],
    remediation_minutes: 1,
    description: r#"Flags type parameters constrained to `any` or `unknown` (`<T extends unknown>`).

**Why:** every type already satisfies `unknown` and `any`, so the constraint restricts nothing. It
only makes the signature longer and suggests to readers that some restriction applies.

**Fix:** remove the `extends` clause: `<T>`.

In `.tsx` files an arrow function's `<T extends unknown>` is a common way to stop the parser from
reading `<T>` as a JSX tag; that form is not flagged (the shorter alternative is `<T,>`).

```ts
function first<T extends unknown>(items: T[]): T {}   // flagged
function first<T>(items: T[]): T {}                   // compliant
function byId<T extends { id: string }>(items: T[]) {} // compliant: a real constraint
```

**Fix available.** `mote fix` deletes `extends unknown` / `extends any`: an unconstrained
type parameter accepts the same types (TypeScript treats `extends any` like `extends unknown`
since 3.9), and types do not exist at run time.
"#,
    fixable: true,
    ..RuleMeta::new(
        "js/no-unnecessary-type-constraint",
        "js",
        "Type parameters should not be constrained to 'any' or 'unknown'",
    )
};

impl JsRule for NoUnnecessaryTypeConstraint {
    fn meta(&self) -> &'static RuleMeta {
        &NO_UNNECESSARY_TYPE_CONSTRAINT
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::TSTypeParameter]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.is_typescript()
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::TSTypeParameter(param) = node.kind() else {
            return;
        };
        let keyword = match &param.constraint {
            Some(TSType::TSAnyKeyword(_)) => "any",
            Some(TSType::TSUnknownKeyword(_)) => "unknown",
            _ => return,
        };
        // `<T extends unknown>() => …` in TSX: the constraint disambiguates the arrow from a tag.
        if ctx.is_jsx() {
            let is_arrow = ctx
                .parent(node)
                .and_then(|decl| ctx.parent_kind(decl))
                .is_some_and(|k| matches!(k, AstKind::ArrowFunctionExpression(_)));
            if is_arrow {
                return;
            }
        }
        let Some(constraint) = &param.constraint else {
            return;
        };
        let between = ctx.text(Span::new(param.name.span.end, constraint.span().start));
        let start = between
            .find("extends")
            .map_or(param.name.span.start, |pos| {
                param.name.span.end + pos as u32
            });
        let commented = ctx.has_comments_in(Span::new(param.name.span.end, constraint.span().end));
        let emitted = ctx.emit(
            Span::new(start, constraint.span().end),
            format!(
                "Remove the constraint 'extends {keyword}' from '{}': every type satisfies it",
                param.name.name
            ),
        );
        if !commented {
            emitted.fix(
                "Delete the constraint",
                vec![Edit::delete(
                    param.name.span.end as usize,
                    constraint.span().end as usize,
                )],
            );
        }
    }
}
