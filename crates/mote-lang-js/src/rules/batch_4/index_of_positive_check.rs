//! `js/index-of-positive-check`

use crate::prelude::*;

pub struct IndexOfPositiveCheck;

pub static INDEX_OF_POSITIVE_CHECK: RuleMeta = RuleMeta {
    severity: Severity::Medium,
    tags: &["suspicious"],
    remediation_minutes: 2,
    description: r#"Flags `indexOf` results tested with `> 0` (or `0 <`).

**Why:** `indexOf` returns `-1` when the value is missing and `0` when it is the very first
element or character. `> 0` therefore treats a match at the start as "not found" — a classic
off-by-one that only shows up for some inputs.

**Fix:** test for presence with `!== -1` (or `>= 0`), or use `includes`. If a match at position 0
really must be excluded, write it out (`index > 0` on a named variable) so readers see it is
deliberate.

```js
if (tags.indexOf('urgent') > 0) {}      // flagged: misses tags[0]

if (tags.indexOf('urgent') !== -1) {}   // compliant
if (tags.includes('urgent')) {}         // compliant
if (path.indexOf('/', 1) > 0) {}        // compliant: search starts after position 0
```

Not reported: searches for a single punctuation character (`name.indexOf(':') > 0`), which usually
mean "a separator after a non-empty prefix" on purpose, and calls with a start position.
"#,
    ..RuleMeta::new(
        "js/index-of-positive-check",
        "js",
        "'indexOf' results should not be tested with '> 0'",
    )
};

impl JsRule for IndexOfPositiveCheck {
    fn meta(&self) -> &'static RuleMeta {
        &INDEX_OF_POSITIVE_CHECK
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::BinaryExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::BinaryExpression(expr) = node.kind() else {
            return;
        };
        let call = match expr.operator {
            BinaryOperator::GreaterThan if is_zero(&expr.right) => &expr.left,
            BinaryOperator::LessThan if is_zero(&expr.left) => &expr.right,
            _ => return,
        };
        if !is_index_of_call(call) {
            return;
        }
        ctx.emit(
            expr.span,
            "Compare the 'indexOf' result with '!== -1' or '>= 0': '> 0' misses a match at position 0",
        );
    }
}

fn is_zero(expr: &Expression<'_>) -> bool {
    matches!(helpers::unwrap_expr(expr), Expression::NumericLiteral(n) if n.value == 0.0)
}

/// `x.indexOf(value)` with exactly one plain argument.
fn is_index_of_call(expr: &Expression<'_>) -> bool {
    let call = match helpers::unwrap_expr(expr) {
        Expression::CallExpression(call) => call,
        Expression::ChainExpression(chain) => match &chain.expression {
            ChainElement::CallExpression(call) => call,
            _ => return false,
        },
        _ => return false,
    };
    let Expression::StaticMemberExpression(member) = helpers::unwrap_expr(&call.callee) else {
        return false;
    };
    let method = member.property.name.as_str();
    if method != "indexOf" || call.arguments.len() != 1 {
        return false;
    }
    let Some(argument) = call.arguments.first().and_then(Argument::as_expression) else {
        return false;
    };
    // `name.indexOf(':') > 0` usually means "a separator after a non-empty prefix": position 0 is
    // excluded on purpose.
    !helpers::string_value(argument).is_some_and(is_separator)
}

/// A single ASCII punctuation character such as `:`, `.`, `/`, `?`, `=`, `@`.
fn is_separator(text: &str) -> bool {
    let mut chars = text.chars();
    matches!((chars.next(), chars.next()), (Some(c), None) if c.is_ascii_punctuation())
}
