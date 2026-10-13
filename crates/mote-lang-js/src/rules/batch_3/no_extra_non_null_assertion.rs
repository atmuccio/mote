//! `js/no-extra-non-null-assertion`

use crate::prelude::*;

pub struct NoExtraNonNullAssertion;

pub static NO_EXTRA_NON_NULL_ASSERTION: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["typescript", "suspicious"],
    remediation_minutes: 2,
    description: r#"Flags non-null assertions that are redundant or contradict the code around them:
a doubled assertion (`value!!`) and an assertion immediately followed by optional chaining
(`value!?.name`, `callback!?.()`).

**Why:** a second `!` adds nothing, and `!` followed by `?.` says two opposite things at once — "this
is never nullish" and "this may be nullish". Usually one of them is a typo or a leftover from an
edit, and a reader cannot tell which behaviour was intended.

**Fix:** keep a single `!`, or — when the value really can be missing — drop the `!` and keep the
optional chaining.

```ts
const size = cache!!.size;      // flagged
const name = user!?.name;       // flagged
onDone!?.();                    // flagged

const size = cache!.size;       // compliant
const name = user?.name;        // compliant
```

**Fix available.** `mote fix` deletes the reported `!`. Non-null assertions are erased when
TypeScript compiles, so the program runs the same; with the optional chaining kept, a `null`
value still short-circuits as before.
"#,
    fixable: true,
    ..RuleMeta::new(
        "js/no-extra-non-null-assertion",
        "js",
        "Non-null assertions should not be doubled or combined with optional chaining",
    )
};

/// A non-null assertion is erased by the compiler: deleting one changes only what the type
/// checker assumes.
fn delete_bang(bang: Span) -> Edit {
    Edit::delete(bang.start as usize, bang.end as usize)
}

impl JsRule for NoExtraNonNullAssertion {
    fn meta(&self) -> &'static RuleMeta {
        &NO_EXTRA_NON_NULL_ASSERTION
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::TSNonNullExpression]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.is_typescript()
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::TSNonNullExpression(expr) = node.kind() else {
            return;
        };
        if matches!(expr.expression, Expression::TSNonNullExpression(_)) {
            if let Some(bang) = super::util::bang_span(ctx, expr.span) {
                ctx.emit(bang, "Remove this repeated non-null assertion")
                    .fix("Delete the `!`", vec![delete_bang(bang)]);
            }
            return;
        }
        let optional_use = match ctx.parent_kind(node) {
            Some(AstKind::StaticMemberExpression(m)) => m.optional && m.object.span() == expr.span,
            Some(AstKind::ComputedMemberExpression(m)) => {
                m.optional && m.object.span() == expr.span
            }
            Some(AstKind::PrivateFieldExpression(m)) => m.optional && m.object.span() == expr.span,
            Some(AstKind::CallExpression(call)) => call.optional && call.callee.span() == expr.span,
            _ => false,
        };
        if optional_use {
            if let Some(bang) = super::util::bang_span(ctx, expr.span) {
                ctx.emit(
                    bang,
                    "Remove either this non-null assertion or the optional chaining that follows it",
                )
                .fix("Delete the `!`", vec![delete_bang(bang)]);
            }
        }
    }
}
