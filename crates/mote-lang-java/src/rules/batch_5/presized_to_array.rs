//! `java/presized-to-array` — `list.toArray(new String[list.size()])`.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["collections", "performance"],
    remediation_minutes: 1,
    description: r#"Detects `c.toArray(new T[c.size()])`: a collection converted to an array
through a pre-sized array.

**Why it matters.** Pre-sizing looks like an optimization but isn't one: current JVMs fill an
empty array (`new T[0]`) at least as fast, because the pre-sized array has to be zeroed before
it is overwritten. The pre-sized form also has a race: on a concurrent or synchronized
collection the size can change between `size()` and `toArray(..)`, leaving `null`s at the
end of the result.

**How to fix.** Pass an empty array (or, on Java 11+, the generator form).

```java
// Noncompliant
String[] names = list.toArray(new String[list.size()]);

// Compliant
String[] names = list.toArray(new String[0]);
String[] names = list.toArray(String[]::new);
```

Only the exact pattern is reported — the array sized by `size()` of the same receiver, or a
map view (`m.keySet()`, `m.values()`, `m.entrySet()`) sized by `m.size()` — and not when the
receiver is known not to be a `java.util.Collection` (for a view: when `m` is not known to be a
`java.util.Map`)."#,
    ..RuleMeta::new(
        "java/presized-to-array",
        "java",
        "Pre-sized array passed to toArray",
    )
};

pub struct PresizedToArray;

impl<'t> JavaRule<'t> for PresizedToArray {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["toArray"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if util::call_named(ctx, node, &["toArray"]).is_none() || util::arg_count(node) != 1 {
            return;
        }
        let Some(receiver) = util::fld(node, F.object) else {
            return;
        };
        let Some(arr) = util::arg(node, 0).map(syntax::strip_parens) else {
            return;
        };
        if kind(arr) != "array_creation_expression" || util::fld(arr, F.value).is_some() {
            return;
        }
        // Exactly one dimension expression: `[c.size()]`.
        let mut dims = named_children(arr).filter(|c| kind(*c) == "dimensions_expr");
        let (Some(dim), None) = (dims.next(), dims.next()) else {
            return;
        };
        let Some(size) = syntax::first_named_child(dim).map(syntax::strip_parens) else {
            return;
        };
        if kind(size) != "method_invocation"
            || util::call_name(ctx, size) != "size"
            || util::arg_count(size) != 0
        {
            return;
        }
        let Some(size_recv) = util::fld(size, F.object) else {
            return;
        };
        if ctx.text(size_recv) == ctx.text(receiver) {
            if let Some(t) = ctx.type_of(receiver) {
                if ctx.subtype(&t, "java.util.Collection") == Some(false) {
                    return;
                }
            }
        } else {
            // `m.keySet().toArray(new K[m.size()])`: a view has the size of its map.
            let view_of = (kind(receiver) == "method_invocation"
                && matches!(
                    util::call_name(ctx, receiver),
                    "keySet" | "values" | "entrySet"
                )
                && util::arg_count(receiver) == 0)
                .then(|| util::fld(receiver, F.object))
                .flatten();
            let Some(map) = view_of else {
                return;
            };
            if ctx.text(map) != ctx.text(size_recv) {
                return;
            }
            let is_map = ctx
                .type_of(map)
                .is_some_and(|t| ctx.is_subtype_of(&t, "java.util.Map"));
            if !is_map {
                return;
            }
        }
        ctx.emit(
            dim,
            "Pass an empty array (`new T[0]`) to `toArray`; pre-sizing it is slower and racy",
        );
    }
}
