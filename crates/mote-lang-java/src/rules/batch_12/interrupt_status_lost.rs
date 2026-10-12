//! `java/interrupt-status-lost` — a handler for `InterruptedException` that neither restores
//! the thread's interrupt status nor propagates the exception.

use super::util::{visit, Walk};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["concurrency", "error-handling"],
    remediation_minutes: 5,
    description: r#"Detects a `catch` block for `InterruptedException` that does something (logs,
counts, retries) but neither calls `interrupt()` to restore the thread's interrupt status nor
throws.

**Why it matters.** Interruption is how Java asks a thread to stop: executors use it to cancel
tasks and shut down, and code further up the stack checks `isInterrupted()` to know it should
wind down. Throwing `InterruptedException` clears the thread's interrupt flag, so a handler that
just logs and carries on erases the request: the loop keeps running, the task ignores its
cancellation, and a shutdown waits for a thread that will never notice it was asked to stop.

**How to fix.** Either let the exception propagate (declare it, or wrap and rethrow it), or
restore the flag with `Thread.currentThread().interrupt()` before continuing, so that the
callers still see the request.

```java
// Noncompliant
while (running) {
    try {
        queue.take().run();
    } catch (InterruptedException e) {
        log.warn("interrupted", e);   // the stop request is lost; the loop goes on
    }
}

// Compliant
while (running) {
    try {
        queue.take().run();
    } catch (InterruptedException e) {
        Thread.currentThread().interrupt();
        return;
    }
}
```

Only handlers that carry on as if nothing happened are reported. Not reported: empty handlers
(left to `empty-catch-block`); handlers that end with `break` or `return` (the code stops what
it was doing) or set a boolean flag (`interrupted = true`, `running = false`) the surrounding
code acts on; handlers that hand the exception to another method or object (other than a
logger) or store it; methods that restore the flag elsewhere (`interrupt()` in a `finally`);
`SwingWorker.done()` methods, where `get()` cannot block; a `try` that is the last thing a
thread's `run()` method does (the thread ends right after); and test code."#,
    ..RuleMeta::new(
        "java/interrupt-status-lost",
        "java",
        "InterruptedException handled without restoring the interrupt status",
    )
};

pub struct InterruptStatusLost;

impl<'t> JavaRule<'t> for InterruptStatusLost {
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
        // Test helpers and sample tests outside the test source roots.
        let file = ctx.path().rsplit('/').next().unwrap_or("");
        if file.contains("Test") {
            return;
        }
        let Some(param) = named_children(node).find(|c| kind(*c) == "catch_formal_parameter")
        else {
            return;
        };
        let Some(types) = named_children(param).find(|c| kind(*c) == "catch_type") else {
            return;
        };
        let Some(ie) = named_children(types).find(|t| {
            let text = ctx.text(*t);
            text == "InterruptedException" || text == "java.lang.InterruptedException"
        }) else {
            return;
        };
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        if syntax::is_empty_block(body) {
            return;
        }
        // A handler that ends the loop or the method reacts to the interruption by stopping.
        if let Some(last) = named_children(body).last() {
            if matches!(kind(last), "break_statement" | "return_statement") {
                return;
            }
        }
        let var = param.field(Field::Name).map(|n| ctx.text(n));
        if handles_interrupt(body, var, ctx) {
            return;
        }
        if let Some(f) = ctx.enclosing_function(node) {
            // `interrupted = true;` … `finally { if (interrupted) Thread.currentThread().interrupt(); }`
            if ctx.text(f).contains(".interrupt()") {
                return;
            }
            // `SwingWorker.done()`: `get()` returns at once, nothing can interrupt it.
            if kind(f) == "method_declaration"
                && f.field(Field::Name).is_some_and(|n| ctx.text(n) == "done")
            {
                return;
            }
            if ends_thread(node, f, ctx) {
                return;
            }
        }
        ctx.emit(
            ie,
            "Restore the interrupt status with Thread.currentThread().interrupt() or rethrow; this handler loses the interruption",
        );
    }
}

/// Method names of logging and printing calls: passing the exception to them does not
/// propagate it.
const LOG_METHODS: &[&str] = &[
    "trace", "debug", "info", "warn", "warning", "error", "fatal", "severe", "fine", "finer",
    "finest", "config", "log", "println", "print", "printf",
];

fn is_logging_call<'t>(call: Node<'t>, ctx: &Ctx<'t>) -> bool {
    let Some(name) = ctx.call_name(call) else {
        return false;
    };
    if !LOG_METHODS.contains(&name) {
        return false;
    }
    let Some(recv) = call.field(Field::Object) else {
        return false;
    };
    let text = ctx.text(recv);
    text == "System.out" || text == "System.err" || text.to_ascii_lowercase().contains("log")
}

/// Whether the handler throws, interrupts a thread, ends the process, or lets the exception
/// escape to code we can't see (argument of a non-logging call, stored, returned).
fn handles_interrupt<'t>(body: Node<'t>, var: Option<&str>, ctx: &Ctx<'t>) -> bool {
    visit(body, |n| match kind(n) {
        "throw_statement" => Walk::Stop,
        // `interrupted = true`, `running = false`: a flag the surrounding code acts on.
        "assignment_expression"
            if n.field(Field::Right)
                .is_some_and(|r| matches!(kind(r), "true" | "false")) =>
        {
            Walk::Stop
        }
        "method_invocation" => match ctx.call_name(n) {
            Some("interrupt" | "exit" | "halt") => Walk::Stop,
            _ => Walk::Continue,
        },
        "identifier" if Some(ctx.text(n)) == var => {
            if escapes(n, ctx) {
                Walk::Stop
            } else {
                Walk::Skip
            }
        }
        _ => Walk::Continue,
    })
}

/// Whether this use of the caught exception may hand it to other code.
fn escapes<'t>(ident: Node<'t>, ctx: &Ctx<'t>) -> bool {
    let Some(parent) = ctx.parent(ident) else {
        return true;
    };
    match kind(parent) {
        // `e.getMessage()`, `e.printStackTrace()`
        "method_invocation" => parent.field(Field::Object).map(|o| o.id()) != Some(ident.id()),
        // `"failed: " + e`
        "binary_expression" => false,
        "argument_list" => match ctx.parent(parent) {
            Some(call) if kind(call) == "method_invocation" => !is_logging_call(call, ctx),
            _ => true,
        },
        _ => true,
    }
}

/// Whether the `try` of this handler is the last thing a thread's `run()` does (only logging
/// may follow): the thread ends right after, so its interrupt status no longer matters.
fn ends_thread<'t>(catch: Node<'t>, func: Node<'t>, ctx: &Ctx<'t>) -> bool {
    if kind(func) != "method_declaration"
        || func.field(Field::Name).is_none_or(|n| ctx.text(n) != "run")
        || syntax::parameters(func).next().is_some()
    {
        return false;
    }
    let (Some(try_stmt), Some(body)) = (ctx.parent(catch), func.field(Field::Body)) else {
        return false;
    };
    if ctx.parent(try_stmt).map(|p| p.id()) != Some(body.id()) {
        return false;
    }
    let mut after = false;
    for stmt in named_children(body) {
        if after {
            let logs = kind(stmt) == "expression_statement"
                && syntax::first_named_child(stmt)
                    .is_some_and(|e| kind(e) == "method_invocation" && is_logging_call(e, ctx));
            if !logs {
                return false;
            }
        } else if stmt.id() == try_stmt.id() {
            after = true;
        }
    }
    after
}
