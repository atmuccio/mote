//! `js/constant-length-comparison`

use super::util;
use crate::prelude::*;

pub struct ConstantLengthComparison;

pub static CONSTANT_LENGTH_COMPARISON: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["suspicious"],
    remediation_minutes: 2,
    description: r#"Flags comparisons of a `.length` with zero or a negative number that can only have one outcome:
`list.length >= 0` (always true), `text.length < 0` (always false), `items.length > -1`.

**Why:** a length is never negative, so these conditions do not test anything. They are almost
always a slip for the real check — `>= 0` written for `> 0` ("is not empty"), or `< 0` copied from
an `indexOf(…) < 0` test — and the guarded code then runs (or is skipped) for empty collections
too.

**Fix:** write the intended test, usually `list.length > 0` or `list.length === 0`.

```js
if (items.length >= 0) render(items);  // flagged: always true
if (name.length < 0) return;           // flagged: always false

if (items.length > 0) render(items);   // compliant
```

Only lengths of values that look like a string or a collection are checked: array and string
literals, results of calls such as `split`, `filter`, or `Object.keys`, and names that are plural
(`items`, `configFiles`) or say what they hold (`list`, `text`, `path`, `args`, …). A record
whose `length` is an ordinary number field (`edit.length`, `range.length`) may well be negative
and is not reported. Neither are optional chains (`list?.length >= 0` is false when `list` is
missing) nor comparisons passed to a call (`assert(buf.length >= 0, 'not array-like')` checks
that a `length` exists at all). The `.size` of a `Map` or `Set` is checked the same way when the
collection is visibly one: a `new Map(…)` / `new Set(…)` expression, or a variable declared in the
file with such a value and never reassigned (`seen.size < 0`).
"#,
    ..RuleMeta::new(
        "js/constant-length-comparison",
        "js",
        "Lengths should not be compared in a way that is always true or always false",
    )
};

impl JsRule for ConstantLengthComparison {
    fn meta(&self) -> &'static RuleMeta {
        &CONSTANT_LENGTH_COMPARISON
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::BinaryExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::BinaryExpression(expr) = node.kind() else {
            return;
        };
        if !matches!(
            expr.operator,
            BinaryOperator::LessThan
                | BinaryOperator::LessEqualThan
                | BinaryOperator::GreaterThan
                | BinaryOperator::GreaterEqualThan
        ) {
            return;
        }
        // Normalize to `<length> <op> <number>`.
        let (length, op, number, is_size) = if let Some(object) = length_object(&expr.left) {
            (
                object,
                expr.operator,
                helpers::number_value(&expr.right),
                false,
            )
        } else if let Some(object) = length_object(&expr.right) {
            let Some(mirrored) = helpers::mirror_comparison(expr.operator) else {
                return;
            };
            (object, mirrored, helpers::number_value(&expr.left), false)
        } else if let Some(object) = size_object(&expr.left) {
            (
                object,
                expr.operator,
                helpers::number_value(&expr.right),
                true,
            )
        } else if let Some(object) = size_object(&expr.right) {
            let Some(mirrored) = helpers::mirror_comparison(expr.operator) else {
                return;
            };
            (object, mirrored, helpers::number_value(&expr.left), true)
        } else {
            return;
        };
        let Some(n) = number else {
            return;
        };
        let always = match op {
            BinaryOperator::GreaterEqualThan if n <= 0.0 => true,
            BinaryOperator::GreaterThan if n < 0.0 => true,
            BinaryOperator::LessThan if n <= 0.0 => false,
            BinaryOperator::LessEqualThan if n < 0.0 => false,
            _ => return,
        };
        let collection = if is_size {
            is_map_or_set(ctx, length)
        } else {
            looks_like_collection(length)
        };
        if !collection || is_call_argument(ctx, node) {
            return;
        }
        let text = util::excerpt(ctx.text(expr.span));
        let what = if is_size { "a size" } else { "a length" };
        ctx.emit(
            expr.span,
            format!(
                "'{text}' is always {always}: {what} is never negative; write the intended check"
            ),
        );
    }
}

/// The object of a (non-optional) `.length` read.
fn length_object<'b, 'a>(expr: &'b Expression<'a>) -> Option<&'b Expression<'a>> {
    match helpers::unwrap_expr(expr) {
        Expression::StaticMemberExpression(m) if m.property.name == "length" && !m.optional => {
            Some(&m.object)
        }
        _ => None,
    }
}

/// The object of a (non-optional) `.size` read.
fn size_object<'b, 'a>(expr: &'b Expression<'a>) -> Option<&'b Expression<'a>> {
    match helpers::unwrap_expr(expr) {
        Expression::StaticMemberExpression(m) if m.property.name == "size" && !m.optional => {
            Some(&m.object)
        }
        _ => None,
    }
}

/// Is the expression visibly a `Map` or `Set`: `new Map(…)` / `new Set(…)` of the globals, or a
/// variable of this file initialized with one and never reassigned?
fn is_map_or_set(ctx: &Ctx<'_>, expr: &Expression<'_>) -> bool {
    let is_new_collection = |e: &Expression<'_>| {
        matches!(helpers::unwrap_expr(e), Expression::NewExpression(new)
            if ctx.is_global(&new.callee, "Map") || ctx.is_global(&new.callee, "Set"))
    };
    match helpers::unwrap_expr(expr) {
        e @ Expression::NewExpression(_) => is_new_collection(e),
        Expression::Identifier(id) => {
            let Some(symbol) = ctx.reference_symbol(id) else {
                return false;
            };
            if ctx.references_of(symbol).any(|r| r.is_write()) {
                return false;
            }
            match ctx.symbol_declaration(symbol).kind() {
                AstKind::VariableDeclarator(d) => {
                    matches!(d.id, BindingPattern::BindingIdentifier(_))
                        && d.init.as_ref().is_some_and(is_new_collection)
                }
                _ => false,
            }
        }
        _ => false,
    }
}

/// Does the object whose `.length` is read look like a string or a collection (never negative),
/// rather than a record with a numeric `length` field (`edit.length`, `range.length`)?
fn looks_like_collection(expr: &Expression<'_>) -> bool {
    match helpers::unwrap_expr(expr) {
        Expression::ArrayExpression(_)
        | Expression::StringLiteral(_)
        | Expression::TemplateLiteral(_) => true,
        Expression::Identifier(id) => collection_name(&id.name),
        Expression::StaticMemberExpression(m) => collection_name(&m.property.name),
        Expression::CallExpression(call) => {
            let name = match &call.callee {
                Expression::StaticMemberExpression(m) => m.property.name.as_str(),
                Expression::Identifier(id) => id.name.as_str(),
                _ => return false,
            };
            matches!(
                name,
                "split"
                    | "filter"
                    | "map"
                    | "flatMap"
                    | "flat"
                    | "slice"
                    | "splice"
                    | "concat"
                    | "keys"
                    | "values"
                    | "entries"
                    | "from"
                    | "trim"
                    | "trimStart"
                    | "trimEnd"
                    | "join"
                    | "substring"
                    | "toLowerCase"
                    | "toUpperCase"
                    | "replace"
                    | "replaceAll"
                    | "querySelectorAll"
                    | "getElementsByTagName"
                    | "getElementsByClassName"
            )
        }
        _ => false,
    }
}

/// A plural name (`items`, `configFiles`) or one that says it holds text or a list.
fn collection_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let plural = lower.len() > 2
        && lower.ends_with('s')
        && !lower.ends_with("ss")
        && !lower.ends_with("us")
        && !lower.ends_with("is");
    plural
        || [
            "list", "array", "arr", "text", "str", "name", "path", "line", "buffer", "queue",
            "stack", "args", "argv", "params", "bytes", "chars", "children", "content", "message",
            "label", "title", "url", "query", "input", "output", "prefix", "suffix", "word",
        ]
        .iter()
        .any(|w| lower.ends_with(w))
}

/// Is the comparison (possibly combined with `&&`, `||`, `!`) an argument of a call? That is the
/// shape of a validity check (`assert(buf.length >= 0, 'must be array-like')`), where the object may
/// not be a collection at all and a missing `length` makes the test fail.
fn is_call_argument(ctx: &Ctx<'_>, node: &AstNode<'_>) -> bool {
    for ancestor in ctx.ancestors(node) {
        match ancestor.kind() {
            AstKind::LogicalExpression(_) | AstKind::UnaryExpression(_) => {}
            AstKind::CallExpression(_) | AstKind::NewExpression(_) => return true,
            _ => return false,
        }
    }
    false
}
