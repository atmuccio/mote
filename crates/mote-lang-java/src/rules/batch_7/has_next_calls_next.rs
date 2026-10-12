//! `java/has-next-calls-next` — an iterator's `hasNext()` that calls its own `next()`.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["iterators", "api-misuse"],
    remediation_minutes: 10,
    description: r#"Detects a `hasNext()` method that calls the same object's `next()` (and
`hasMoreElements()` calling `nextElement()`).

**Why it matters.** `hasNext()` must be a side-effect-free query: callers may call it any
number of times before `next()`. Implementing it by fetching the next element consumes that
element, so every call to `hasNext()` silently skips one — `for`-each loops lose every other
element, and `hasNext(); hasNext();` runs off the end.

**How to fix.** Look ahead without consuming: compute and cache the next element in a field
(`hasNext()` fills it if empty, `next()` returns and clears it).

```java
// Noncompliant
public boolean hasNext() {
    return next() != null;
}

// Compliant
public boolean hasNext() {
    if (pending == null) {
        pending = fetch();
    }
    return pending != null;
}
```

Only unqualified or `this.` calls to a `next()` declared in the same class are reported; calls
on another iterator (delegation) are fine."#,
    ..RuleMeta::new("java/has-next-calls-next", "java", "hasNext() calls next()")
};

pub struct HasNextCallsNext;

impl<'t> JavaRule<'t> for HasNextCallsNext {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_declaration"]
    }

    fn method_names(&self) -> &'static [&'static str] {
        &["hasNext", "hasMoreElements"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name_node) = node.field(Field::Name) else {
            return;
        };
        let next = match ctx.text(name_node) {
            "hasNext" => "next",
            "hasMoreElements" => "nextElement",
            _ => return,
        };
        if syntax::parameters(node).next().is_some() {
            return;
        }
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        if !ctx.text(body).contains(next) {
            return;
        }
        let Some(class_body) = ctx.parent(node) else {
            return;
        };
        let own_next = ctx.members(class_body).any(|m| {
            kind(m) == "method_declaration"
                && syntax::name_of(m, ctx.source()) == next
                && syntax::parameters(m).next().is_none()
                && !syntax::has_modifier(m, "static")
        });
        if !own_next {
            return;
        }
        let call = util::find_in_scope(body, |n| {
            kind(n) == "method_invocation"
                && ctx.call_name(n) == Some(next)
                && syntax::argument_count(n) == 0
                && n.field(Field::Object).is_none_or(|o| kind(o) == "this")
        });
        if let Some(call) = call {
            ctx.emit(
                call,
                format!("Don't call `{next}()` from `{}()`: it consumes an element each time the caller only asks whether one exists", ctx.text(name_node)),
            );
        }
    }
}
