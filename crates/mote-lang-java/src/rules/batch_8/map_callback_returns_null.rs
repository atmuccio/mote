//! `java/map-callback-returns-null` — a `Map.computeIfAbsent/compute/merge` callback that
//! always returns `null`, as if `null` were stored.

use super::util::{self, Walk};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["collections", "null", "api-misuse"],
    remediation_minutes: 5,
    description: r#"Detects `Map` callbacks that always return `null` where that cannot be what the
author wants:

* a `computeIfAbsent` mapping function that always returns `null`: the map never stores
  anything, so the call is an expensive `get`;
* a `compute`, `computeIfPresent` or `merge` function that changes the current value (adds
  to it, puts into it, appends to it) and then returns `null`: returning `null` *removes* the
  entry, so the change is thrown away together with it.

**Why it matters.** For these methods `null` is not a value that can be stored: it means
"no mapping". Code written as if the callback's side effects or its `null` were kept loses
data silently, and the next lookup finds nothing.

**How to fix.** Return the value to store (the new or the updated object). To remove an
entry, call `remove` so the intent is visible.

```java
// Noncompliant
index.compute(key, (k, list) -> {
    list.add(item);
    return null;                  // removes the entry, item is lost
});

// Compliant
index.computeIfAbsent(key, k -> new ArrayList<>()).add(item);
```

Only callbacks written as lambdas whose every `return` is the `null` literal, on receivers the
resolver proves to be a `java.util.Map`, are considered. A `compute` function that returns
`null` after closing or releasing the old value (an atomic remove-and-clean-up) is not
reported."#,
    ..RuleMeta::new(
        "java/map-callback-returns-null",
        "java",
        "Map callback always returns null",
    )
};

pub struct MapCallbackReturnsNull;

/// Methods that change a collection, map or builder they are called on.
const MUTATORS: &[&str] = &[
    "add",
    "addAll",
    "put",
    "putAll",
    "append",
    "offer",
    "push",
    "addFirst",
    "addLast",
    "set",
    "increment",
    "incrementAndGet",
    "getAndIncrement",
    "addAndGet",
    "getAndAdd",
    "merge",
    "compute",
    "computeIfAbsent",
    "insert",
    "addElement",
];

/// Whether every `return` of the lambda body is `return null;` and the body cannot complete
/// normally without one (it ends with it).
fn always_null(body: Node<'_>) -> bool {
    if kind(body) != "block" {
        return syntax::is_null_literal(body);
    }
    let Some(last) = named_children(body).last() else {
        return false;
    };
    let is_return_null = |r: Node<'_>| {
        kind(r) == "return_statement"
            && syntax::first_named_child(r).is_some_and(syntax::is_null_literal)
    };
    if !is_return_null(last) {
        return false;
    }
    !util::visit(body, |n| {
        if util::is_nested_scope(n) {
            Walk::Skip
        } else if kind(n) == "return_statement" && !is_return_null(n) {
            Walk::Stop
        } else {
            Walk::Continue
        }
    })
}

/// Whether the body calls a mutating method on the value parameter `value`.
fn mutates<'t>(ctx: &Ctx<'t>, body: Node<'t>, value: &str) -> bool {
    util::any_in_scope(body, |n| {
        kind(n) == "method_invocation"
            && n.field(Field::Object)
                .is_some_and(|o| kind(o) == "identifier" && ctx.text(o) == value)
            && n.field(Field::Name)
                .is_some_and(|m| MUTATORS.contains(&ctx.text(m)))
    })
}

/// The lambda's parameter names.
fn lambda_params<'t>(ctx: &Ctx<'t>, lambda: Node<'t>) -> Vec<&'t str> {
    let Some(p) = lambda.field(Field::Parameters) else {
        return Vec::new();
    };
    match kind(p) {
        "identifier" => vec![ctx.text(p)],
        "inferred_parameters" => named_children(p)
            .filter(|c| kind(*c) == "identifier")
            .map(|c| ctx.text(c))
            .collect(),
        "formal_parameters" => named_children(p)
            .filter_map(|c| c.field(Field::Name))
            .map(|c| ctx.text(c))
            .collect(),
        _ => Vec::new(),
    }
}

impl<'t> JavaRule<'t> for MapCallbackReturnsNull {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["computeIfAbsent", "compute", "computeIfPresent", "merge"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) = ctx.call_name(node) else {
            return;
        };
        let (arity, value_index) = match name {
            "computeIfAbsent" => (2, None),
            "compute" | "computeIfPresent" => (2, Some(1)),
            "merge" => (3, Some(0)),
            _ => return,
        };
        if util::arg_count(node) != arity {
            return;
        }
        let Some(lambda) = util::arg(node, arity - 1) else {
            return;
        };
        if kind(lambda) != "lambda_expression" {
            return;
        }
        let Some(body) = lambda.field(Field::Body) else {
            return;
        };
        if !ctx.text(body).contains("null") || !always_null(body) {
            return;
        }
        let message = match value_index {
            None => "This mapping function always returns null, so `computeIfAbsent` never stores anything",
            Some(i) => {
                let params = lambda_params(ctx, lambda);
                let Some(value) = params.get(i) else { return };
                if kind(body) != "block" || !mutates(ctx, body, value) {
                    return;
                }
                "Returning null removes the entry: the change made to the current value is lost"
            }
        };
        if !ctx.is_call_to(node, "java.util.Map", name) {
            return;
        }
        ctx.emit(node.field(Field::Name).unwrap_or(node), message);
    }
}
