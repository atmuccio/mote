//! `java/log-and-rethrow` — a catch block that logs the exception and then throws it on.

use super::util::{self, LogLevel};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["error-handling", "logging"],
    remediation_minutes: 5,
    description: r#"Detects a catch block that writes the caught exception (or its message) to a
logger at info level or above, and ends by throwing it on: the exception itself (`throw e`)
or a new exception that carries it as its cause (`throw new ServiceException("…", e)`).
Both the logging call and the `throw` are statements of the catch block itself.

**Why it matters.** An exception that keeps propagating will be handled somewhere up the
stack, and that handler usually logs it too. Logging it at every layer it crosses fills the log
with the same failure several times, often with the full stack trace each time, which makes it
look like several problems and buries the one line that matters. The layer that finally deals
with the failure is the one that knows how serious it is and should be the one that logs it.

**How to fix.** Either handle the exception here and log it, or let it propagate (add context
by wrapping it) and leave the logging to whoever handles it.

```java
// Noncompliant
try {
    store.save(order);
} catch (IOException e) {
    log.error("could not save order {}", order.id(), e);
    throw new StorageException("could not save order " + order.id(), e);
}

// Compliant: the message travels with the exception and is logged once, where it is handled
try {
    store.save(order);
} catch (IOException e) {
    throw new StorageException("could not save order " + order.id(), e);
}
```

Loggers are recognized by their declared type (SLF4J, Log4j, Commons Logging, JBoss Logging,
`java.util.logging`) or by a conventional name in a file importing a logging API. Not
reported: debug and trace output (diagnostics that are off in production), a rethrow that
only happens on some paths (`if (fatal) throw e;` — the handler decided which failures to
pass on, and may add context for them), logging inside such a branch, exceptions thrown
without the caught one as their cause (see `java/lost-exception-cause`), and throws inside
lambdas or nested `try` statements."#,
    ..RuleMeta::new(
        "java/log-and-rethrow",
        "java",
        "Exception logged and rethrown",
    )
};

pub struct LogAndRethrow;

/// Whether the thrown expression passes the caught exception `name` on: `e`, `(X) e`, or a new
/// exception taking `e` as a direct argument.
fn rethrows<'t>(ctx: &Ctx<'t>, thrown: Node<'t>, name: &str) -> bool {
    let thrown = syntax::strip_parens(thrown);
    match kind(thrown) {
        "identifier" => ctx.text(thrown) == name,
        "cast_expression" => thrown
            .field(Field::Value)
            .map(syntax::strip_parens)
            .is_some_and(|v| kind(v) == "identifier" && ctx.text(v) == name),
        "object_creation_expression" => {
            // An anonymous exception subclass is unusual enough to leave alone.
            if syntax::child_of_kind(thrown, "class_body").is_some() {
                return false;
            }
            util::args(thrown).any(|a| {
                let a = syntax::strip_parens(a);
                kind(a) == "identifier" && ctx.text(a) == name
            })
        }
        _ => false,
    }
}

/// The logging call of statement `stmt` that mentions `name` among its arguments, when it logs
/// at info level or above.
fn log_call_with<'t>(ctx: &Ctx<'t>, stmt: Node<'t>, name: &str) -> Option<Node<'t>> {
    if kind(stmt) != "expression_statement" {
        return None;
    }
    let call = syntax::first_named_child(stmt)?;
    if kind(call) != "method_invocation" {
        return None;
    }
    let args = call.field(Field::Arguments)?;
    if !util::mentions(ctx, args, name) {
        return None;
    }
    let level = util::log_call_level(ctx, call)?;
    (level >= LogLevel::Info).then_some(call)
}

impl<'t> JavaRule<'t> for LogAndRethrow {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["catch_clause"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        let text = ctx.text(body);
        if !text.contains("throw") {
            return;
        }
        let Some(name) = util::catch_param_name(node, ctx) else {
            return;
        };
        if !text.contains(name) {
            return;
        }
        // The handler ends by throwing the exception on, whatever happened before: a rethrow
        // on some paths only means the handler decided which failures to pass on.
        let Some(last) = named_children(body).last() else {
            return;
        };
        if kind(last) != "throw_statement" {
            return;
        }
        let Some(thrown) = syntax::first_named_child(last) else {
            return;
        };
        if !rethrows(ctx, thrown, name) {
            return;
        }
        let log = named_children(body)
            .take_while(|s| s.id() != last.id())
            .find_map(|s| log_call_with(ctx, s, name));
        if let Some(log) = log {
            ctx.emit(
                log,
                format!(
                    "Either log `{name}` here or throw it on, not both: whoever handles it will \
                     log it again"
                ),
            )
            .secondary(last, "rethrown here");
        }
    }
}
