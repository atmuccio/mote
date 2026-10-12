//! `java/collection-passed-to-itself` — `list.addAll(list)`, `set.retainAll(set)`,
//! `c.containsAll(c)`, `map.putAll(map)`.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["collections", "suspicious", "copy-paste"],
    remediation_minutes: 2,
    description: r#"Detects a collection or map passed as the argument of one of its own bulk
methods: `c.addAll(c)`, `c.removeAll(c)`, `c.retainAll(c)`, `c.containsAll(c)`,
`m.putAll(m)`.

**Why it matters.** The result is fixed and rarely what was meant: `containsAll(self)` is
always `true`, `retainAll(self)` and `putAll(self)` change nothing, `removeAll(self)` empties
the collection (a slow `clear()`), and `addAll(self)` duplicates every element of a list (a
set is left unchanged; some implementations throw `ConcurrentModificationException`). It is
almost always a copy-paste slip for another variable (`merged.addAll(other)`).

**How to fix.** Pass the collection that was meant, or call `clear()` if that was the intent.

```java
// Noncompliant
allowed.retainAll(allowed);

// Compliant
allowed.retainAll(granted);
```

Receivers must be plain names or field references (no calls), and the resolver must prove
they are a `Collection` or a `Map`."#,
    ..RuleMeta::new(
        "java/collection-passed-to-itself",
        "java",
        "Collection passed to its own bulk method",
    )
};

pub struct CollectionPassedToItself;

impl<'t> JavaRule<'t> for CollectionPassedToItself {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["addAll", "removeAll", "retainAll", "containsAll", "putAll"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) = ctx.call_name(node) else {
            return;
        };
        let Some(recv) = node.field(Field::Object) else {
            return;
        };
        let mut args = util::args(node);
        let (Some(arg), None) = (args.next(), args.next()) else {
            return;
        };
        let (r, a) = (syntax::strip_parens(recv), syntax::strip_parens(arg));
        if !util::is_plain_ref(r) || ctx.text(r) != ctx.text(a) || kind(r) != kind(a) {
            return;
        }
        let (owner, effect) = match name {
            "addAll" => (
                "java.util.Collection",
                "duplicates a list's elements (and does nothing for a set)",
            ),
            "removeAll" => ("java.util.Collection", "empties the collection"),
            "retainAll" => ("java.util.Collection", "changes nothing"),
            "containsAll" => ("java.util.Collection", "is always true"),
            "putAll" => ("java.util.Map", "changes nothing"),
            _ => return,
        };
        let Some(t) = ctx.type_of(r) else {
            return;
        };
        if !ctx.is_subtype_of(&t, owner) {
            return;
        }
        ctx.emit(
            node,
            format!(
                "Pass a different collection: `{}` passed to its own `{name}` {effect}",
                ctx.text(r)
            ),
        );
    }
}
