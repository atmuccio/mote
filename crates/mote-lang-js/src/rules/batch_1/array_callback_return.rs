//! `js/array-callback-return`

use super::util;
use crate::prelude::*;

pub struct ArrayCallbackReturn;

pub static ARRAY_CALLBACK_RETURN: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["suspicious"],
    remediation_minutes: 5,
    description: r#"Flags block-bodied callbacks passed to array methods that use the callback's return value
(`map`, `filter`, `find`, `findIndex`, `findLast`, `findLastIndex`, `some`, `every`, `reduce`,
`reduceRight`, `flatMap`, `sort`, `toSorted`, and `Array.from`'s mapper) when the callback never
returns a value.

**Why:** these methods are defined by what the callback returns. A callback that only has side
effects makes `map` produce an array of `undefined`, `filter` and `find` match nothing, `sort`
leave the order unspecified, and `reduce` lose its accumulator. It usually means a `return` was
forgotten when an arrow function was converted to a block body — or that `forEach` was meant.

**Fix:** return the value, or use `forEach` / a `for … of` loop when only the side effects matter.

```js
const names = users.map((u) => {
  u.name.trim();             // flagged: nothing is returned
});

const names = users.map((u) => {
  return u.name.trim();      // compliant
});

users.forEach((u) => { log(u); }); // compliant: forEach ignores return values
```

Async and generator callbacks, callbacks that always throw, and callbacks passed in any position
other than the first are not checked. Empty callbacks (`() => {}`), `map` callbacks that take no
parameter (mapping an event or observable to void), and callbacks whose second parameter is named
`cb`, `callback`, `done`, or `next` (node-style async iterators such as `es.map`) are deliberate
and not reported either.
"#,
    ..RuleMeta::new(
        "js/array-callback-return",
        "js",
        "Array method callbacks should return a value",
    )
};

const METHODS: &[&str] = &[
    "map",
    "filter",
    "find",
    "findIndex",
    "findLast",
    "findLastIndex",
    "some",
    "every",
    "reduce",
    "reduceRight",
    "flatMap",
    "sort",
    "toSorted",
];

/// Callee names the rule acts on: [`METHODS`] and `Array.from`.
const CALLEES: &[&str] = &[
    "map",
    "filter",
    "find",
    "findIndex",
    "findLast",
    "findLastIndex",
    "some",
    "every",
    "reduce",
    "reduceRight",
    "flatMap",
    "sort",
    "toSorted",
    "from",
];

impl JsRule for ArrayCallbackReturn {
    fn meta(&self) -> &'static RuleMeta {
        &ARRAY_CALLBACK_RETURN
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CallExpression]
    }

    fn callee_names(&self) -> &'static [&'static str] {
        CALLEES
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::CallExpression(call) = node.kind() else {
            return;
        };
        let Expression::StaticMemberExpression(member) = helpers::unwrap_expr(&call.callee) else {
            return;
        };
        let method = member.property.name.as_str();
        let index = if METHODS.contains(&method) {
            0
        } else if method == "from" && ctx.is_global(&member.object, "Array") {
            1
        } else {
            return;
        };
        let Some(callback) = call.arguments.get(index).and_then(Argument::as_expression) else {
            return;
        };
        let (body, params) = match helpers::unwrap_expr(callback) {
            Expression::ArrowFunctionExpression(f) if !f.r#async && !f.expression => {
                (&f.body, &f.params)
            }
            Expression::FunctionExpression(f) if !f.r#async && !f.generator => match &f.body {
                Some(body) => (body, &f.params),
                None => return,
            },
            _ => return,
        };
        // `() => {}` is a deliberate no-op, and `.map(() => { … })` that ignores its input maps a
        // stream to void (event and observable APIs), not an array.
        if body.statements.is_empty() || (method == "map" && params.items.is_empty()) {
            return;
        }
        // `(item, cb) => …`: a node-style async iterator (`es.map`, `async.filter`), which
        // reports its result through the callback.
        if params.items.get(1).is_some_and(|p| {
            p.pattern
                .get_identifier_name()
                .is_some_and(|n| matches!(n.as_str(), "cb" | "callback" | "done" | "next"))
        }) {
            return;
        }
        let (value, bare) = util::return_kinds(&body.statements);
        // No `return` at all and no normal completion: the callback always throws.
        if value || (!bare && util::list_exits(&body.statements)) {
            return;
        }
        ctx.emit(
            member.property.span,
            format!("Return a value from this '{method}' callback, or use 'forEach' if only side effects are needed"),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callee_names_cover_the_rule() {
        for name in METHODS.iter().chain(&["from"]) {
            assert!(CALLEES.contains(name), "{name} missing from CALLEES");
        }
    }
}
