//! `js/no-global-assign`

use crate::prelude::*;

pub struct NoGlobalAssign;

pub static NO_GLOBAL_ASSIGN: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["suspicious"],
    remediation_minutes: 5,
    description: r#"Flags assignments to built-in globals of the language: `undefined = 0`, `NaN = …`,
`Object = …`, `Promise = MyPromise`, `JSON = …`, and the other standard objects and functions
(also `window` and `document` in browsers).

**Why:** `undefined`, `NaN`, `Infinity`, `window`, and `document` cannot be changed: the assignment
is silently ignored in sloppy code and throws in strict code. Replacing a standard object such as
`Object`, `Array`, or `Promise` changes it for every script and library on the page, which breaks
code far away from the assignment. It is usually a missing declaration (`let Map = …` was meant)
or a typo.

**Fix:** declare a local variable with another name (or with `let`/`const` if a local was meant).

```js
Promise = CustomPromise;          // flagged: replaces Promise everywhere
undefined = null;                 // flagged: has no effect, or throws

const TaskPromise = CustomPromise; // compliant
```

Not reported: names declared in the file (they are not the globals), and polyfills that assign only
when the built-in is missing (`if (typeof Promise === 'undefined') Promise = …`, `Map = Map || …`).
"#,
    ..RuleMeta::new(
        "js/no-global-assign",
        "js",
        "Built-in globals should not be reassigned",
    )
};

const BUILT_INS: &[&str] = &[
    "undefined",
    "NaN",
    "Infinity",
    "globalThis",
    "Object",
    "Array",
    "String",
    "Number",
    "Boolean",
    "Symbol",
    "BigInt",
    "Function",
    "Math",
    "JSON",
    "Reflect",
    "Proxy",
    "Intl",
    "Promise",
    "Map",
    "Set",
    "WeakMap",
    "WeakSet",
    "WeakRef",
    "Date",
    "RegExp",
    "Error",
    "TypeError",
    "RangeError",
    "SyntaxError",
    "ReferenceError",
    "EvalError",
    "URIError",
    "AggregateError",
    "ArrayBuffer",
    "SharedArrayBuffer",
    "DataView",
    "Int8Array",
    "Uint8Array",
    "Uint8ClampedArray",
    "Int16Array",
    "Uint16Array",
    "Int32Array",
    "Uint32Array",
    "Float32Array",
    "Float64Array",
    "BigInt64Array",
    "BigUint64Array",
    "parseInt",
    "parseFloat",
    "isNaN",
    "isFinite",
    "encodeURI",
    "encodeURIComponent",
    "decodeURI",
    "decodeURIComponent",
    "window",
    "document",
];

impl JsRule for NoGlobalAssign {
    fn meta(&self) -> &'static RuleMeta {
        &NO_GLOBAL_ASSIGN
    }

    fn run_once<'a>(&self, ctx: &mut Ctx<'a>) {
        let scoping = ctx.scoping();
        let mut reports: Vec<(Span, &'static str)> = Vec::new();
        for (name, ids) in scoping.root_unresolved_references() {
            let Some(&builtin) = BUILT_INS.iter().find(|b| **b == name.as_str()) else {
                continue;
            };
            for &id in ids {
                let reference = scoping.get_reference(id);
                if !reference.is_write() {
                    continue;
                }
                let node = ctx.node(reference.node_id());
                if ctx.in_ambient_context(node) || is_polyfill(ctx, node, builtin) {
                    continue;
                }
                reports.push((node.span(), builtin));
            }
        }
        reports.sort_by_key(|r| r.0.start);
        for (span, name) in reports {
            ctx.emit(
                span,
                format!("Do not assign to the built-in global '{name}': declare a variable with another name"),
            );
        }
    }
}

/// `Map = Map || Polyfill` or an assignment inside `if (typeof Map === 'undefined')` /
/// `if (!window.Map)`: the built-in is only set where it is missing.
fn is_polyfill(ctx: &Ctx<'_>, node: &AstNode<'_>, name: &str) -> bool {
    for ancestor in ctx.ancestors(node) {
        let text = match ancestor.kind() {
            AstKind::AssignmentExpression(a) => ctx.text(a.right.span()),
            AstKind::IfStatement(s) => ctx.text(s.test.span()),
            AstKind::ConditionalExpression(c) => ctx.text(c.test.span()),
            AstKind::LogicalExpression(l) => ctx.text(l.left.span()),
            kind if kind.is_function_like() => return false,
            _ => continue,
        };
        if mentions_word(text, name) {
            return true;
        }
    }
    false
}

/// Does `text` contain `name` as a whole word?
fn mentions_word(text: &str, name: &str) -> bool {
    text.match_indices(name).any(|(i, _)| {
        let before = text[..i].chars().next_back();
        let after = text[i + name.len()..].chars().next();
        let is_ident =
            |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$');
        !is_ident(before) && !is_ident(after)
    })
}
