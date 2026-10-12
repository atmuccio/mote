//! `js/use-isnan`

use crate::prelude::*;

pub struct UseIsnan;

pub static USE_ISNAN: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["suspicious"],
    remediation_minutes: 2,
    description: r#"Flags comparisons with `NaN` (`x === NaN`, `x != Number.NaN`, `x < NaN`) and `switch`
statements that use `NaN` as the discriminant or as a `case`.

**Why:** `NaN` is the only JavaScript value that is not equal to itself, and every comparison
involving it is `false` (or `true` for `!=`/`!==`). So `x === NaN` never matches, even when `x`
*is* `NaN`, and `case NaN:` can never be selected.

**Fix:** use `Number.isNaN(x)` (or `isNaN(x)` when a conversion to number is wanted).

```js
if (value === NaN) {}         // flagged: always false
if (Number.isNaN(value)) {}   // compliant
```
"#,
    ..RuleMeta::new(
        "js/use-isnan",
        "js",
        "Use 'Number.isNaN()' to check for NaN",
    )
};

impl JsRule for UseIsnan {
    fn meta(&self) -> &'static RuleMeta {
        &USE_ISNAN
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::BinaryExpression, AstType::SwitchStatement]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        match node.kind() {
            AstKind::BinaryExpression(expr) => {
                if !helpers::is_comparison(expr.operator) {
                    return;
                }
                if is_nan(ctx, &expr.left) || is_nan(ctx, &expr.right) {
                    ctx.emit(
                        expr.span,
                        "Use 'Number.isNaN()' instead of comparing with NaN: this comparison never matches",
                    );
                }
            }
            AstKind::SwitchStatement(switch) => {
                if is_nan(ctx, &switch.discriminant) {
                    ctx.emit(
                        switch.discriminant.span(),
                        "Replace 'switch (NaN)' with 'Number.isNaN()' checks: no case can match NaN",
                    );
                }
                for case in &switch.cases {
                    if let Some(test) = &case.test {
                        if is_nan(ctx, test) {
                            ctx.emit(
                                test.span(),
                                "Replace 'case NaN' with a 'Number.isNaN()' check: it can never match",
                            );
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// The global `NaN` or `Number.NaN`.
fn is_nan(ctx: &Ctx<'_>, expr: &Expression<'_>) -> bool {
    let expr = helpers::unwrap_expr(expr);
    if ctx.is_global(expr, "NaN") {
        return true;
    }
    match expr {
        Expression::StaticMemberExpression(m) => {
            m.property.name == "NaN" && ctx.is_global(&m.object, "Number")
        }
        Expression::ComputedMemberExpression(m) => {
            helpers::string_value(&m.expression) == Some("NaN")
                && ctx.is_global(&m.object, "Number")
        }
        _ => false,
    }
}
