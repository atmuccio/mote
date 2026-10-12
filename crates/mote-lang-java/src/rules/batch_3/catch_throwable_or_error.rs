//! `java/catch-throwable-or-error` — `catch (Throwable t)` / `catch (Error e)`.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["error-handling"],
    remediation_minutes: 10,
    description: r#"Detects `catch` clauses for `java.lang.Throwable` or `java.lang.Error`
(also as one alternative of a multi-catch).

**Why it matters.** `Error`s signal conditions an application usually cannot recover from:
`OutOfMemoryError`, `StackOverflowError`, `LinkageError`, a failed assertion. Catching them
(directly or through `Throwable`) and carrying on leaves the JVM in a doubtful state and hides
the real problem behind whatever the handler does next.

**How to fix.** Catch the exceptions the protected code can actually throw, typically
`Exception` subclasses. If a last-resort handler is really needed (the top of a worker thread),
log and rethrow, or let the default uncaught-exception handler do its job.

```java
// Noncompliant
try {
    process(batch);
} catch (Throwable t) {
    log.warn("batch failed", t);
}

// Compliant
try {
    process(batch);
} catch (IOException | ParseException e) {
    log.warn("batch failed", e);
}
```

Handlers that always end by throwing (rethrowing or wrapping the error, so it keeps
propagating) are not reported: a final `throw`, or a final `if`/`else` whose branches all
end that way. Neither is a `Throwable` clause that follows a `catch (Error e)` clause of the
same `try`, which has already taken every `Error`."#,
    ..RuleMeta::new(
        "java/catch-throwable-or-error",
        "java",
        "Catching Throwable or Error",
    )
};

pub struct CatchThrowableOrError;

/// Whether a block (or statement) always ends by throwing: its last statement is a `throw`, a
/// block that does, or an `if` with an `else` whose branches both do.
fn always_throws(stmt: Node<'_>, depth: u32) -> bool {
    if depth > 8 {
        return false;
    }
    match kind(stmt) {
        "throw_statement" => true,
        "block" => util::statements(stmt)
            .last()
            .is_some_and(|s| always_throws(s, depth + 1)),
        "if_statement" => {
            stmt.field(Field::Consequence)
                .is_some_and(|c| always_throws(c, depth + 1))
                && stmt
                    .field(Field::Alternative)
                    .is_some_and(|a| always_throws(a, depth + 1))
        }
        _ => false,
    }
}

/// Whether an earlier `catch` clause of the same `try` catches `java.lang.Error`, so that this
/// clause never receives one.
fn error_caught_before<'t>(ctx: &Ctx<'t>, clause: Node<'t>) -> bool {
    let Some(try_stmt) = ctx.parent(clause) else {
        return false;
    };
    named_children(try_stmt)
        .take_while(|c| c.id() != clause.id())
        .filter(|c| kind(*c) == "catch_clause")
        .any(|c| {
            util::catch_type(c).is_some_and(|types| {
                named_children(types).any(|t| {
                    util::type_simple_name(t, ctx) == "Error"
                        && util::type_fqn(ctx, t).as_deref() == Some("java.lang.Error")
                })
            })
        })
}

impl<'t> JavaRule<'t> for CatchThrowableOrError {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["catch_clause"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(types) = util::catch_type(node) else {
            return;
        };
        for t in named_children(types) {
            let simple = util::type_simple_name(t, ctx);
            if !matches!(simple, "Throwable" | "Error") {
                continue;
            }
            let Some(fqn) = util::type_fqn(ctx, t) else {
                continue;
            };
            if !matches!(fqn.as_str(), "java.lang.Throwable" | "java.lang.Error") {
                continue;
            }
            if node.field(Field::Body).is_some_and(|b| always_throws(b, 0)) {
                return;
            }
            if simple == "Throwable" && error_caught_before(ctx, node) {
                return;
            }
            ctx.emit(
                t,
                format!("Catch specific exceptions instead of `{simple}`, which also traps fatal JVM errors"),
            );
        }
    }
}
