//! `java/catch-generic-exception` — `catch (Exception e)` / `catch (RuntimeException e)`.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["error-handling"],
    remediation_minutes: 10,
    description: r#"Detects `catch` clauses for `java.lang.Exception` or
`java.lang.RuntimeException`.

**Why it matters.** A catch-all handler written for one expected failure also swallows every
other one: a `NullPointerException`, an `IllegalStateException` from a bug, an exception added
to a callee later. They all get the same treatment, usually a log line, and programming errors
turn into silently wrong behaviour.

**How to fix.** Catch the specific exception types the code in the `try` block is expected to
throw (use a multi-catch for several), and let unexpected ones propagate.

```java
// Noncompliant
try {
    config = parser.parse(file);
} catch (Exception e) {
    config = Config.defaults();
}

// Compliant
try {
    config = parser.parse(file);
} catch (IOException | ParseException e) {
    config = Config.defaults();
}
```

Not reported: handlers whose last statement is a `throw` (the exception keeps propagating,
usually wrapped because the callee declares `throws Exception`), empty handlers (with or without
a comment; left to `java/empty-catch-block`), `try` blocks calling JDK methods declared
`throws Exception`
(`AutoCloseable.close()`, `Callable.call()`), and test code."#,
    ..RuleMeta::new(
        "java/catch-generic-exception",
        "java",
        "Catching a generic exception",
    )
};

pub struct CatchGenericException;

/// JDK methods declared `throws Exception`: a caller has no narrower type to catch.
const THROWS_EXCEPTION: &[(&str, &str)] = &[
    ("java.lang.AutoCloseable", "close"),
    ("java.util.concurrent.Callable", "call"),
];

/// Whether the `try` of this catch clause calls a JDK method declared `throws Exception`
/// (`AutoCloseable.close()`, `Callable.call()`) or manages a plain `AutoCloseable` resource.
fn try_forces_exception<'t>(catch: Node<'t>, ctx: &Ctx<'t>) -> bool {
    let Some(try_stmt) = ctx.parent(catch) else {
        return false;
    };
    if let Some(res) = try_stmt.field(Field::Resources) {
        let plain = named_children(res).any(|r| {
            r.field(Field::Type)
                .and_then(|t| ctx.resolve_type(t))
                .is_some_and(|t| t.is_class("java.lang.AutoCloseable"))
        });
        if plain {
            return true;
        }
    }
    let Some(body) = try_stmt.field(Field::Body) else {
        return false;
    };
    util::any_descendant(body, util::is_other_context, |n| {
        kind(n) == "method_invocation"
            && util::name_field(n).is_some_and(|m| {
                let m = ctx.text(m);
                THROWS_EXCEPTION
                    .iter()
                    .any(|(owner, name)| *name == m && ctx.is_call_to(n, owner, name))
            })
    })
}

impl<'t> JavaRule<'t> for CatchGenericException {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["catch_clause"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if ctx.is_test() {
            return;
        }
        let Some(types) = util::catch_type(node) else {
            return;
        };
        // Cheap syntactic exclusions before resolving any type.
        if node
            .field(Field::Body)
            .is_none_or(|b| util::block_ends_with_throw(b) || syntax::is_empty_block(b))
        {
            return;
        }
        for t in named_children(types) {
            let simple = util::type_simple_name(t, ctx);
            if !matches!(simple, "Exception" | "RuntimeException") {
                continue;
            }
            let Some(fqn) = util::type_fqn(ctx, t) else {
                continue;
            };
            if !matches!(
                fqn.as_str(),
                "java.lang.Exception" | "java.lang.RuntimeException"
            ) {
                continue;
            }
            if simple == "Exception" && try_forces_exception(node, ctx) {
                return;
            }
            ctx.emit(
                t,
                format!("Catch the specific exceptions expected here instead of `{simple}`"),
            );
        }
    }
}
