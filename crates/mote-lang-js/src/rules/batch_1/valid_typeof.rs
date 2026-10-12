//! `js/valid-typeof`

use crate::prelude::*;

pub struct ValidTypeof;

pub static VALID_TYPEOF: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["suspicious"],
    remediation_minutes: 2,
    description: r#"Flags `typeof` comparisons against a string that `typeof` can never return, such as
`typeof x === 'strnig'`, `typeof x == 'array'`, or `typeof x === undefined`.

**Why:** `typeof` only ever yields `"undefined"`, `"object"`, `"boolean"`, `"number"`,
`"bigint"`, `"string"`, `"symbol"`, or `"function"`. A comparison with anything else is always
false (or always true for `!==`), so the check silently does nothing — typically because of a typo
or a wrong assumption (arrays and `null` are `"object"`). Comparing with the value `undefined`
instead of the string `"undefined"` has the same effect.

**Fix:** compare with one of the valid strings; use `Array.isArray(x)` for arrays and
`x === null` for null.

```js
if (typeof value === 'fucntion') {}   // flagged
if (typeof value === undefined) {}    // flagged: typeof returns a string
if (typeof value === 'function') {}   // compliant
```
"#,
    ..RuleMeta::new(
        "js/valid-typeof",
        "js",
        "'typeof' should be compared with a valid type name",
    )
};

const VALID: &[&str] = &[
    "undefined",
    "object",
    "boolean",
    "number",
    "bigint",
    "string",
    "symbol",
    "function",
];

impl JsRule for ValidTypeof {
    fn meta(&self) -> &'static RuleMeta {
        &VALID_TYPEOF
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::BinaryExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::BinaryExpression(expr) = node.kind() else {
            return;
        };
        if !expr.operator.is_equality() {
            return;
        }
        let other = if helpers::is_typeof(&expr.left) {
            &expr.right
        } else if helpers::is_typeof(&expr.right) {
            &expr.left
        } else {
            return;
        };
        if let Some(value) = helpers::string_value(other) {
            if !VALID.contains(&value) {
                let shown: String = value.chars().take(40).collect();
                ctx.emit(
                    other.span(),
                    format!("Replace '{shown}': 'typeof' never returns this value"),
                );
            }
        } else if ctx.is_global(other, "undefined") {
            ctx.emit(
                other.span(),
                "Compare with the string 'undefined': 'typeof' always returns a string",
            );
        }
    }
}
