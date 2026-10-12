//! `js/no-undeclared-assignment`

use super::util;
use crate::prelude::*;

pub struct NoUndeclaredAssignment;

pub static NO_UNDECLARED_ASSIGNMENT: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["suspicious", "globals"],
    remediation_minutes: 2,
    description: r#"Flags assignments to a name that is declared nowhere: not in the file, not as a known
browser or Node.js global, and not in a `/* global … */` comment.

**Why:** assigning to an undeclared name does not create a local variable. In sloppy-mode scripts it
silently creates (or overwrites) a property of the global object, shared by every script on the page
and every call of the function — a classic source of state leaking between calls and of clashes
with other code. In strict mode and in ES modules the same statement throws a `ReferenceError` at
runtime. Almost always, a `let`, `const`, or `var` was forgotten.

**Fix:** declare the variable where it is first assigned. If a global really is intended, write it
explicitly as `globalThis.name = …` (or `window.name`) and declare it with a `/* global name */`
comment where it is read.

```js
function total(items) {
  sum = 0;                    // flagged: 'sum' is never declared
  for (const item of items) sum += item.price;
  return sum;
}

function total(items) {
  let sum = 0;                // compliant
  for (const item of items) sum += item.price;
  return sum;
}
```

To stay precise, a name is only reported when everything the file does with it happens inside one
function and starts with the assignment (a variable used like a local), when it is not a known
global (`window`, `location`, `module`, `exports`, `on…` handlers, …), and when the file does not
test it with `typeof`, list it in a `/* global */` comment, or `declare` it. TypeScript files are
not checked: the compiler already rejects assignments to names that no declaration file declares.
Files using `with` or direct `eval` are skipped.
"#,
    ..RuleMeta::new(
        "js/no-undeclared-assignment",
        "js",
        "Variables should be declared before they are assigned",
    )
};

/// One reference to a name: source offset, node, and flags.
type Use = (u32, NodeId, ReferenceFlags);

/// Globals that code commonly assigns on purpose (browser window properties, Node.js/CommonJS,
/// workers, AMD, and well-known library/test globals).
const KNOWN_GLOBALS: &[&str] = &[
    // Browser window.
    "window",
    "self",
    "top",
    "parent",
    "opener",
    "frames",
    "location",
    "name",
    "status",
    "defaultStatus",
    "closed",
    "length",
    "event",
    "document",
    "navigator",
    "history",
    "console",
    "performance",
    "localStorage",
    "sessionStorage",
    "innerWidth",
    "innerHeight",
    "outerWidth",
    "outerHeight",
    "scrollX",
    "scrollY",
    "pageXOffset",
    "pageYOffset",
    "screenX",
    "screenY",
    "devicePixelRatio",
    "origin",
    "fetch",
    "XMLHttpRequest",
    "WebSocket",
    "Worker",
    "postMessage",
    "close",
    "importScripts",
    // Node.js / CommonJS / AMD.
    "global",
    "globalThis",
    "module",
    "exports",
    "require",
    "process",
    "Buffer",
    "__dirname",
    "__filename",
    "define",
    "requirejs",
    // Built-ins that polyfills replace.
    "Promise",
    "Symbol",
    "Map",
    "Set",
    "WeakMap",
    "WeakSet",
    "Proxy",
    "Reflect",
    "Intl",
    "Array",
    "Object",
    "String",
    "Number",
    "Boolean",
    "Date",
    "RegExp",
    "Error",
    "JSON",
    "Math",
    // Libraries and test runners.
    "$",
    "jQuery",
    "_",
    "angular",
    "Vue",
    "React",
    "ReactDOM",
    "ko",
    "Backbone",
    "moment",
    "d3",
    "google",
    "ga",
    "gtag",
    "dataLayer",
    "jasmine",
    "expect",
    "assert",
    "should",
    "chai",
    "sinon",
    "describe",
    "it",
    "test",
];

impl JsRule for NoUndeclaredAssignment {
    fn meta(&self) -> &'static RuleMeta {
        &NO_UNDECLARED_ASSIGNMENT
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        !ctx.is_typescript()
            && !ctx.file_contains(AstType::WithStatement)
            && !util::uses_global(ctx, "eval")
    }

    fn run_once<'a>(&self, ctx: &mut Ctx<'a>) {
        let scoping = ctx.scoping();
        let mut candidates: Vec<(&'a str, Vec<Use>)> = Vec::new();
        for (name, ids) in scoping.root_unresolved_references() {
            let name = name.as_str();
            if is_known_global(name) {
                continue;
            }
            let mut refs: Vec<Use> = ids
                .iter()
                .map(|&id| {
                    let reference = scoping.get_reference(id);
                    let node = reference.node_id();
                    (ctx.node(node).span().start, node, reference.flags())
                })
                .collect();
            if !refs.iter().any(|r| r.2.is_write()) {
                continue;
            }
            refs.sort_unstable_by_key(|r| r.0);
            candidates.push((name, refs));
        }
        if candidates.is_empty() {
            return;
        }
        let declared = declared_elsewhere(ctx);
        let mut reports: Vec<(Span, &'a str)> = Vec::new();
        for (name, refs) in candidates {
            if declared.contains(name) {
                continue;
            }
            let Some(&(_, first, flags)) = refs.first() else {
                continue;
            };
            // Used like a local: the first use is a plain write (whose value does not read the
            // name itself, as in `hits = hits + 1`) …
            if flags != ReferenceFlags::Write {
                continue;
            }
            if let Some(assignment) = ctx
                .ancestors(first)
                .find(|n| matches!(n.kind(), AstKind::AssignmentExpression(_)))
            {
                let span = assignment.span();
                if refs
                    .iter()
                    .any(|&(at, _, f)| f.is_read() && span.start <= at && at < span.end)
                {
                    continue;
                }
            }
            // … inside a function that contains every other use.
            let Some(function) = ctx.enclosing_function(first) else {
                continue;
            };
            let body = function.span();
            if refs
                .iter()
                .any(|&(at, _, _)| at < body.start || at >= body.end)
            {
                continue;
            }
            if refs
                .iter()
                .any(|&(_, node, _)| is_typeof_operand(ctx, node))
            {
                continue;
            }
            reports.push((ctx.node(first).span(), name));
        }
        reports.sort_unstable_by_key(|r| r.0.start);
        for (span, name) in reports {
            ctx.emit(
                span,
                format!(
                    "Declare '{name}' with 'let' or 'const': assigning an undeclared name creates a global variable (or throws in strict mode)"
                ),
            );
        }
    }
}

fn is_known_global(name: &str) -> bool {
    // Event handler properties: `onload`, `onmessage`, `onerror`, …
    let handler = name.len() > 2
        && name.starts_with("on")
        && name.as_bytes()[2..].iter().all(u8::is_ascii_lowercase);
    handler || KNOWN_GLOBALS.contains(&name)
}

fn is_typeof_operand(ctx: &Ctx<'_>, node: NodeId) -> bool {
    ctx.ancestors(node)
        .find(|n| !matches!(n.kind(), AstKind::IdentifierReference(_)))
        .is_some_and(|n| {
            matches!(n.kind(), AstKind::UnaryExpression(u) if u.operator == UnaryOperator::Typeof)
        })
}

/// Names declared for the linter or the type checker in this file: `/* global a, b:writable */`
/// and `/* globals … */` comments, and ambient (`declare`) declarations.
fn declared_elsewhere<'a>(ctx: &Ctx<'a>) -> FxHashSet<&'a str> {
    let mut names = FxHashSet::default();
    for comment in ctx.comments() {
        let text = ctx.text(comment.span);
        let Some(body) = text.strip_prefix("/*") else {
            continue;
        };
        let body = body.strip_suffix("*/").unwrap_or(body).trim_start();
        let list = body
            .strip_prefix("globals")
            .or_else(|| body.strip_prefix("global"));
        let Some(list) = list else {
            continue;
        };
        if !list.starts_with(char::is_whitespace) {
            continue;
        }
        for item in list.split(|c: char| c == ',' || c.is_whitespace()) {
            let name = item.split(':').next().unwrap_or("").trim();
            if !name.is_empty() {
                names.insert(name);
            }
        }
    }
    let scoping = ctx.scoping();
    for symbol in scoping.symbol_ids() {
        if scoping.symbol_flags(symbol).contains(SymbolFlags::Ambient) {
            names.insert(scoping.symbol_name(symbol));
        }
    }
    names
}
