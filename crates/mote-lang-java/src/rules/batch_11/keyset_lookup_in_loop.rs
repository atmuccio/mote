//! `java/keyset-lookup-in-loop` — iterating `map.keySet()` and calling `map.get(key)` for each
//! key instead of iterating `map.entrySet()`.

use super::util::{self, Walk};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["performance", "collections"],
    remediation_minutes: 5,
    description: r#"Detects a for-each loop over `map.keySet()` whose body looks each key up again
with `map.get(key)` on the same map.

**Why it matters.** The loop already walks the map's entries to produce the keys; looking
each one up again repeats the work — a hash computation and bucket search per key for a
`HashMap`, a tree descent for a `TreeMap`. Iterating `entrySet()` gives the key and the value
together, reads more directly, and for a `TreeMap` turns an `n log n` loop into a linear one.

**How to fix.** Iterate the entries and use `getKey()` / `getValue()` (or `forEach((k, v) ->
…)`).

```java
// Noncompliant
for (String name : settings.keySet()) {
    apply(name, settings.get(name));
}

// Compliant
for (Map.Entry<String, String> e : settings.entrySet()) {
    apply(e.getKey(), e.getValue());
}
```

Only maps the resolver proves to be a `java.util.Map` are considered (multimaps and other
map-like types, where `get` and `entrySet` mean something else, are left alone). Not reported:
lookups in another map, lookups with anything other than the loop variable itself, and
lookups inside lambdas or anonymous classes in the body."#,
    ..RuleMeta::new(
        "java/keyset-lookup-in-loop",
        "java",
        "Map values looked up while iterating its keys",
    )
};

pub struct KeysetLookupInLoop;

/// `m` / `this.m` as a comparable text (`this.` dropped).
fn map_ref<'t>(ctx: &Ctx<'t>, expr: Node<'t>) -> Option<&'t str> {
    let expr = syntax::strip_parens(expr);
    match kind(expr) {
        "identifier" => Some(ctx.text(expr)),
        "field_access" => {
            let obj = expr.field(Field::Object)?;
            let field = expr.field(Field::Field)?;
            (kind(obj) == "this" && kind(field) == "identifier").then(|| ctx.text(field))
        }
        _ => None,
    }
}

impl<'t> JavaRule<'t> for KeysetLookupInLoop {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["enhanced_for_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(value) = node.field(Field::Value).map(syntax::strip_parens) else {
            return;
        };
        if kind(value) != "method_invocation"
            || ctx.call_name(value) != Some("keySet")
            || syntax::argument_count(value) != 0
        {
            return;
        }
        let (Some(map), Some(var), Some(body)) = (
            value.field(Field::Object),
            node.field(Field::Name),
            node.field(Field::Body),
        ) else {
            return;
        };
        let Some(map_name) = map_ref(ctx, map) else {
            return;
        };
        let key = ctx.text(var);
        let body_text = ctx.text(body);
        if !body_text.contains(".get(") || !body_text.contains(key) {
            return;
        }
        let mut lookup = None;
        util::visit(body, |n| {
            if util::is_other_context(n) {
                return Walk::Skip;
            }
            if kind(n) == "method_invocation"
                && ctx.call_name(n) == Some("get")
                && n.field(Field::Object)
                    .and_then(|o| map_ref(ctx, o))
                    .is_some_and(|m| m == map_name)
                && syntax::arguments(n).count() == 1
                && syntax::arguments(n)
                    .next()
                    .map(syntax::strip_parens)
                    .is_some_and(|a| kind(a) == "identifier" && ctx.text(a) == key)
            {
                lookup = Some(n);
                return Walk::Stop;
            }
            Walk::Continue
        });
        let Some(lookup) = lookup else {
            return;
        };
        // The loop variable must not be reassigned (then `get` would look up another key).
        let reassigned = util::visit(body, |n| match kind(n) {
            "assignment_expression" => {
                if n.field(Field::Left)
                    .is_some_and(|l| kind(l) == "identifier" && ctx.text(l) == key)
                {
                    Walk::Stop
                } else {
                    Walk::Continue
                }
            }
            _ => Walk::Continue,
        });
        if reassigned {
            return;
        }
        let Some(map_type) = ctx.type_of(map) else {
            return;
        };
        if !ctx.is_subtype_of(&map_type, "java.util.Map") {
            return;
        }
        ctx.emit(
            value,
            format!(
                "Iterate over `{map_name}.entrySet()` instead of looking each key up again with \
                 `{map_name}.get({key})`"
            ),
        )
        .secondary(lookup, "second lookup");
    }
}
