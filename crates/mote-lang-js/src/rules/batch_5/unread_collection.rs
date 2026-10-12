//! `js/unread-collection`

use super::util;
use crate::prelude::*;

pub struct UnreadCollection;

pub static UNREAD_COLLECTION: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["unused", "suspicious"],
    remediation_minutes: 5,
    description: r#"Flags a local array, `Map`, or `Set` that the code fills (`push`, `add`, `set`, `a[i] = …`) but
never reads.

**Why:** building a collection nobody looks at is wasted work, and it usually means something was
forgotten: the results were meant to be returned, rendered, or checked, but the final step is
missing — or the collection is left over from an earlier version of the code.

**Fix:** use the collection where it was meant to be used, or remove it together with the code
that fills it.

```js
function validate(rows) {
  const errors = [];                 // flagged: filled but never read
  for (const row of rows) {
    if (!row.id) errors.push(`row without id`);
  }
  return true;
}

function validate(rows) {
  const errors = [];                 // compliant
  for (const row of rows) {
    if (!row.id) errors.push(`row without id`);
  }
  return errors;
}
```

Only `const`/`let` variables initialized with an array literal or `new Map/Set/WeakMap/WeakSet/
Array(…)` are considered, and only when every use is visible: a collection that is exported,
passed to a function, returned, stored elsewhere, reassigned, or used through any other method is
not reported. Top-level variables of classic scripts (globals) are skipped.
"#,
    ..RuleMeta::new(
        "js/unread-collection",
        "js",
        "Collections that are filled should be read",
    )
};

impl JsRule for UnreadCollection {
    fn meta(&self) -> &'static RuleMeta {
        &UNREAD_COLLECTION
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::VariableDeclarator]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::VariableDeclarator(declarator) = node.kind() else {
            return;
        };
        let Some((symbol, _)) = util::local_collection(ctx, node, declarator) else {
            return;
        };
        let usage = util::collection_use(ctx, symbol);
        if usage.escapes
            || !usage.reads.is_empty()
            || usage.spreads > 0
            || usage.discarded_mutations == 0
        {
            return;
        }
        let name = ctx.symbol_name(symbol);
        ctx.emit(
            ctx.symbol_span(symbol),
            format!("'{name}' is filled but never read: use its contents or remove it"),
        );
    }
}
