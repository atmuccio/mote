//! `js/no-non-null-assertion`

use crate::prelude::*;

pub struct NoNonNullAssertion;

pub static NO_NON_NULL_ASSERTION: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["typescript", "conventions"],
    remediation_minutes: 5,
    description: r#"Flags the TypeScript non-null assertion operator (`value!`).

**Why:** `!` tells the compiler "trust me, this is never `null` or `undefined`" without any check at
runtime. When the assumption is wrong — often after a later refactoring — the code fails far away
from the real cause with `Cannot read properties of undefined`. The assertion also hides the case
from readers, who cannot see why the value is guaranteed to exist.

**Fix:** handle the missing case explicitly: narrow with a check (`if (!value) { … }`), use optional
chaining (`value?.name`) or a default (`value ?? fallback`), or fix the declared type so that it is
not nullable in the first place.

Test files are not checked: there, a wrong assumption makes the test fail on the spot, which is
exactly what an assertion in a test is for.

```ts
const name = user!.name;                 // flagged

if (!user) {
  throw new Error('no current user');
}
const name = user.name;                  // compliant: narrowed by the check
const label = user?.name ?? 'anonymous'; // compliant
```
"#,
    // Stylistic and noisy on real code: opt-in (`extends: all` or enable explicitly).
    default_enabled: false,
    ..RuleMeta::new(
        "js/no-non-null-assertion",
        "js",
        "Non-null assertions should not be used",
    )
};

impl JsRule for NoNonNullAssertion {
    fn meta(&self) -> &'static RuleMeta {
        &NO_NON_NULL_ASSERTION
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::TSNonNullExpression]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        // In tests a wrong assumption fails the test right there, which is what the test wants.
        ctx.is_typescript() && !ctx.is_test()
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::TSNonNullExpression(expr) = node.kind() else {
            return;
        };
        // `x!!` is reported once here (at the outer `!`); the doubled `!` itself is
        // `js/no-extra-non-null-assertion`'s business.
        if matches!(ctx.parent_kind(node), Some(AstKind::TSNonNullExpression(_))) {
            return;
        }
        let Some(bang) = super::util::bang_span(ctx, expr.span) else {
            return;
        };
        ctx.emit(
            bang,
            "Replace this non-null assertion with an explicit check, optional chaining, or a default",
        );
    }
}
