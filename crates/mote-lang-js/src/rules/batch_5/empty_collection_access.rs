//! `js/empty-collection-access`

use super::util;
use crate::prelude::*;

pub struct EmptyCollectionAccess;

pub static EMPTY_COLLECTION_ACCESS: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["suspicious"],
    remediation_minutes: 5,
    description: r#"Flags reads of a local array, `Map`, or `Set` that starts empty and is never filled: iterating
it, looking something up, or checking its length.

**Why:** a collection that nothing ever adds to is always empty, so every read of it has a fixed
answer — loops over it never run, lookups always miss, `.length` is always `0`. This is usually a
bug: the code that was supposed to fill it was lost, or it fills a different variable with a
similar name.

**Fix:** fill the collection where intended (often a missing `push`/`set`), read the right
variable, or remove the dead code.

```js
const seen = new Set();
for (const user of users) {
  if (seen.has(user.id)) continue;   // flagged: seen is always empty
  notify(user);
}

const seen = new Set();
for (const user of users) {
  if (seen.has(user.id)) continue;   // compliant
  seen.add(user.id);
  notify(user);
}
```

Only `const`/`let` variables declared inside a function and initialized with `[]` or
`new Map()`/`new Set()`/`new Array()` without arguments are considered, and only when every use
is visible: a collection that is exported, passed to a function, returned, stored elsewhere,
reassigned, or used through an unknown method is not reported. Top-level constants are skipped
(an empty list there is often a placeholder filled in by hand later), and so are literals holding
a comment (usually commented-out entries). Spreading an empty list (`[...defaults, ...extra]`)
is not counted as a read: tests often pass an empty list on purpose.
"#,
    ..RuleMeta::new(
        "js/empty-collection-access",
        "js",
        "Collections that are never filled should not be read",
    )
};

impl JsRule for EmptyCollectionAccess {
    fn meta(&self) -> &'static RuleMeta {
        &EMPTY_COLLECTION_ACCESS
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::VariableDeclarator]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::VariableDeclarator(declarator) = node.kind() else {
            return;
        };
        let empty_literal = match declarator.init.as_ref().map(helpers::unwrap_expr) {
            Some(Expression::ArrayExpression(a)) => a.elements.is_empty(),
            Some(Expression::NewExpression(n)) => n.arguments.is_empty(),
            _ => false,
        };
        if !empty_literal {
            return;
        }
        // Function-local only: an empty top-level list is often a placeholder filled in by hand
        // later, and comments inside the literal usually hold commented-out entries.
        if !ctx.is_in_function(node)
            || declarator
                .init
                .as_ref()
                .is_some_and(|init| ctx.has_comments_in(init.span()))
        {
            return;
        }
        let Some((symbol, true)) = util::local_collection(ctx, node, declarator) else {
            return;
        };
        let usage = util::collection_use(ctx, symbol);
        if usage.escapes || usage.mutations > 0 {
            return;
        }
        let Some(&first) = usage.reads.iter().min_by_key(|s| s.start) else {
            return;
        };
        let name = ctx.symbol_name(symbol);
        let declared = ctx.symbol_span(symbol);
        ctx.emit(
            first,
            format!("'{name}' is always empty here: nothing is ever added to it"),
        )
        .secondary(declared, "Declared empty here");
    }
}
