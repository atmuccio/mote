//! `js/ignored-call-result`

use super::util::{identifier_kind, ValueKind as Kind};
use crate::prelude::*;

pub struct IgnoredCallResult;

pub static IGNORED_CALL_RESULT: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["suspicious", "unused"],
    remediation_minutes: 2,
    description: r#"Flags a statement that calls a string or array method which returns a new value and changes
nothing — `name.trim();`, `list.concat(more);`, `text.replace(/x/g, '');` — so the result is lost.

**Why:** strings are immutable, and methods such as `concat`, `slice`, or `toSorted` leave the
array alone. The statement has no effect: the author almost certainly expected the method to
update the value in place, and the code that follows still sees the old value.

**Fix:** use the result — `name = name.trim();` — or, for arrays, call the in-place method
(`push`, `sort`, `splice`).

```ts
function clean(name: string, list: number[]) {
  name.trim();               // flagged: the trimmed string is thrown away
  list.concat([4]);          // flagged
  name = name.trim();        // compliant
  list.push(4);              // compliant
}
```

Without type information only receivers that are visibly strings or arrays are checked: literals,
`String(…)`, results of other string methods on those, and variables or parameters declared with a
`string` / array type annotation or initialized with a literal and never reassigned. Methods that
take a callback (`map`, `filter`, …) are left alone, since the callback may be the point of the
call (a `replace` with a function as replacement is skipped for the same reason).
"#,
    ..RuleMeta::new(
        "js/ignored-call-result",
        "js",
        "Results of string and array methods that return a new value should be used",
    )
};

/// String methods without side effects.
const STRING_METHODS: &[&str] = &[
    "at",
    "charAt",
    "charCodeAt",
    "codePointAt",
    "concat",
    "endsWith",
    "includes",
    "indexOf",
    "lastIndexOf",
    "localeCompare",
    "match",
    "matchAll",
    "normalize",
    "padEnd",
    "padStart",
    "repeat",
    "replace",
    "replaceAll",
    "search",
    "slice",
    "split",
    "startsWith",
    "substr",
    "substring",
    "toLocaleLowerCase",
    "toLocaleUpperCase",
    "toLowerCase",
    "toUpperCase",
    "trim",
    "trimEnd",
    "trimLeft",
    "trimRight",
    "trimStart",
];

/// String methods that return a string (so a chain on them is still a string).
const STRING_RESULT: &[&str] = &[
    "concat",
    "normalize",
    "padEnd",
    "padStart",
    "repeat",
    "replace",
    "replaceAll",
    "slice",
    "substr",
    "substring",
    "toLocaleLowerCase",
    "toLocaleUpperCase",
    "toLowerCase",
    "toUpperCase",
    "trim",
    "trimEnd",
    "trimLeft",
    "trimRight",
    "trimStart",
];

/// Array methods without side effects that take no callback.
const ARRAY_METHODS: &[&str] = &[
    "at",
    "concat",
    "entries",
    "flat",
    "includes",
    "indexOf",
    "join",
    "keys",
    "lastIndexOf",
    "slice",
    "toReversed",
    "toSorted",
    "toSpliced",
    "values",
    "with",
];

/// A filter of [`STRING_METHODS`] and [`ARRAY_METHODS`].
static METHODS: crate::helpers::NameFilter = {
    let mut both = [""; STRING_METHODS.len() + ARRAY_METHODS.len()];
    let mut i = 0;
    while i < STRING_METHODS.len() {
        both[i] = STRING_METHODS[i];
        i += 1;
    }
    let mut j = 0;
    while j < ARRAY_METHODS.len() {
        both[i + j] = ARRAY_METHODS[j];
        j += 1;
    }
    crate::helpers::NameFilter::of(&both)
};

impl JsRule for IgnoredCallResult {
    fn meta(&self) -> &'static RuleMeta {
        &IGNORED_CALL_RESULT
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::ExpressionStatement]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::ExpressionStatement(stmt) = node.kind() else {
            return;
        };
        let call = match helpers::unwrap_expr(&stmt.expression) {
            Expression::CallExpression(call) => call,
            Expression::ChainExpression(chain) => match &chain.expression {
                ChainElement::CallExpression(call) => call,
                _ => return,
            },
            _ => return,
        };
        let Expression::StaticMemberExpression(member) = helpers::unwrap_expr(&call.callee) else {
            return;
        };
        let method = member.property.name.as_str();
        // Runs on every call statement: most method names are rejected without a list scan.
        if !METHODS.may_contain(method) {
            return;
        }
        let in_string = STRING_METHODS.contains(&method);
        let in_array = ARRAY_METHODS.contains(&method);
        if !in_string && !in_array {
            return;
        }
        // `text.replace(re, (m) => { found.push(m); })` walks the matches through the callback.
        if matches!(method, "replace" | "replaceAll")
            && call
                .arguments
                .get(1)
                .and_then(Argument::as_expression)
                .is_none_or(|e| helpers::string_value(helpers::unwrap_expr(e)).is_none())
        {
            return;
        }
        // `() => s.trim()`: the expression body is the function's result.
        if let Some(body) = ctx.parent(node) {
            if matches!(body.kind(), AstKind::FunctionBody(_))
                && matches!(ctx.parent_kind(body), Some(AstKind::ArrowFunctionExpression(a)) if a.expression)
            {
                return;
            }
        }
        let Some(kind) = receiver_kind(ctx, &member.object, 0) else {
            return;
        };
        if (kind == Kind::String && !in_string) || (kind == Kind::Array && !in_array) {
            return;
        }
        let noun = if kind == Kind::String {
            "strings"
        } else {
            "the array"
        };
        let message = if kind == Kind::String {
            format!("Use the result of '{method}()': {noun} cannot be changed in place, so this call has no effect")
        } else {
            format!("Use the result of '{method}()': it returns a new value and leaves {noun} unchanged, so this call has no effect")
        };
        ctx.emit(member.property.span, message);
    }
}

/// Is the expression visibly a string or an array?
fn receiver_kind(ctx: &Ctx<'_>, expr: &Expression<'_>, depth: usize) -> Option<Kind> {
    match helpers::unwrap_expr(expr) {
        Expression::StringLiteral(_) | Expression::TemplateLiteral(_) => Some(Kind::String),
        Expression::ArrayExpression(_) => Some(Kind::Array),
        Expression::CallExpression(call) => {
            if ctx.is_global(&call.callee, "String") {
                return Some(Kind::String);
            }
            let Expression::StaticMemberExpression(m) = helpers::unwrap_expr(&call.callee) else {
                return None;
            };
            if depth < 4
                && STRING_RESULT.contains(&m.property.name.as_str())
                && receiver_kind(ctx, &m.object, depth + 1) == Some(Kind::String)
            {
                return Some(Kind::String);
            }
            None
        }
        Expression::Identifier(id) => identifier_kind(ctx, id),
        _ => None,
    }
}
