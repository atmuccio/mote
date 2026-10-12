//! `java/assertion-in-thread` — a test assertion inside code that runs on another thread,
//! where its failure never reaches the test.

use super::util::{self, Walk};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["tests", "concurrency"],
    remediation_minutes: 10,
    description: r#"Detects test assertions (JUnit, AssertJ, Hamcrest, TestNG, Truth) inside a
lambda or anonymous class that runs on another thread whose failure is never collected: the
body of a `new Thread(..)`, a task given to `Executor.execute(..)`, and a task given to
`ExecutorService.submit(..)` or `CompletableFuture.runAsync/supplyAsync(..)` whose returned
future is thrown away.

**Why it matters.** A failed assertion throws an `AssertionError` in the thread that runs it.
In a plain thread or an `execute`d task, that error goes to the thread's uncaught-exception
handler (a stack trace on the console); in a discarded future it is stored and never looked
at. Either way the test method itself carries on and passes: the assertion looks like
protection but cannot fail the test.

**How to fix.** Bring the outcome back to the test thread: keep the `Future` and call `get()`
(which rethrows the failure), collect errors in a thread-safe list and assert on it after
`join()`, or compute the value in the task and assert on it in the test.

```java
// Noncompliant
executor.submit(() -> assertEquals(200, client.call().status()));

// Compliant
Future<?> f = executor.submit(() -> assertEquals(200, client.call().status()));
f.get();
```

Only test files are checked. Not reported: assertions guarded by a `catch` of `Throwable`,
`Error` or `AssertionError` inside the task, assertions in a `catch` block that also hands the
caught error to a collector (`errors.add(e)`, `failure.set(e)`: the recommended fix, the test
checks the collection after `join()`), tasks that do more work after the assertion (a latch
count-down or flag after it makes a failure visible to the test as a timeout), and files that
install an uncaught-exception handler."#,
    ..RuleMeta::new(
        "java/assertion-in-thread",
        "java",
        "Assertion inside a thread body",
    )
};

#[derive(Default)]
pub struct AssertionInThread {
    /// The file mentions an assertion and installs no uncaught-exception handler.
    open: Option<bool>,
}

/// The code that runs on the other thread: a lambda body or the `run`/`call` method body of
/// an anonymous class.
fn task_body<'t>(ctx: &Ctx<'t>, arg: Node<'t>) -> Option<Node<'t>> {
    match kind(arg) {
        "lambda_expression" => arg.field(Field::Body),
        "object_creation_expression" => anonymous_run(ctx, arg),
        _ => None,
    }
}

fn anonymous_run<'t>(ctx: &Ctx<'t>, creation: Node<'t>) -> Option<Node<'t>> {
    let body = syntax::type_body(creation)?;
    ctx.members(body)
        .find(|m| {
            kind(*m) == "method_declaration"
                && matches!(syntax::name_of(*m, ctx.source()), "run" | "call")
                && syntax::parameters(*m).next().is_none()
        })
        .and_then(|m| m.field(Field::Body))
}

fn is_assertion_call<'t>(ctx: &Ctx<'t>, n: Node<'t>) -> bool {
    if kind(n) != "method_invocation" {
        return false;
    }
    let Some(name) = n.field(Field::Name).map(|x| ctx.text(x)) else {
        return false;
    };
    (name.starts_with("assert") || name == "fail") && util::is_test_library_call(ctx, n, name)
}

/// Whether `n` sits inside a `try` (within `root`) that catches `Throwable`, `Error` or
/// `AssertionError`.
fn caught_inside<'t>(ctx: &Ctx<'t>, n: Node<'t>, root: Node<'t>) -> bool {
    for a in ctx.ancestors(n) {
        if a.id() == root.id() {
            return false;
        }
        if matches!(kind(a), "try_statement" | "try_with_resources_statement") {
            let catches = named_children(a).filter(|c| kind(*c) == "catch_clause");
            for c in catches {
                let text = named_children(c)
                    .find(|p| kind(*p) == "catch_formal_parameter")
                    .map(|p| ctx.text(p))
                    .unwrap_or("");
                if [
                    "Throwable",
                    "Error",
                    "AssertionError",
                    "AssertionFailedError",
                ]
                .iter()
                .any(|t| {
                    text.split(|ch: char| !ch.is_alphanumeric())
                        .any(|w| w == *t)
                }) {
                    return true;
                }
            }
        }
    }
    false
}

/// Whether the task does anything but more assertions after the statement ending at `end`.
fn work_after<'t>(ctx: &Ctx<'t>, body: Node<'t>, end: usize) -> bool {
    util::visit(body, |n| {
        if n.end_byte() <= end {
            return Walk::Skip;
        }
        if n.start_byte() < end {
            return Walk::Continue;
        }
        match kind(n) {
            "expression_statement" => {
                let is_assert =
                    syntax::first_named_child(n).is_some_and(|e| is_assertion_call(ctx, e));
                if is_assert {
                    Walk::Skip
                } else {
                    Walk::Stop
                }
            }
            "local_variable_declaration"
            | "return_statement"
            | "throw_statement"
            | "yield_statement" => Walk::Stop,
            _ => Walk::Continue,
        }
    })
}

impl AssertionInThread {
    fn open(&mut self, ctx: &Ctx<'_>) -> bool {
        *self.open.get_or_insert_with(|| {
            let src = ctx.source();
            (src.contains("assert") || src.contains("fail("))
                && !src.contains("UncaughtExceptionHandler")
        })
    }

    fn check_task<'t>(&self, ctx: &Ctx<'t>, arg: Node<'t>) {
        let Some(body) = task_body(ctx, arg) else {
            return;
        };
        if !ctx.text(body).contains("ssert") && !ctx.text(body).contains("fail") {
            return;
        }
        let mut found = is_assertion_call(ctx, body).then_some(body);
        util::visit(body, |n| {
            if found.is_some() {
                return Walk::Stop;
            }
            if matches!(kind(n), "class_body") {
                return Walk::Skip;
            }
            if kind(n) == "catch_clause" && handler_collects_error(ctx, n) {
                return Walk::Skip;
            }
            if is_assertion_call(ctx, n) {
                found = Some(n);
                return Walk::Stop;
            }
            Walk::Continue
        });
        let Some(assertion) = found else { return };
        if caught_inside(ctx, assertion, body) {
            return;
        }
        let stmt_end = ctx
            .ancestors(assertion)
            .take_while(|a| a.id() != body.id())
            .find(|a| kind(*a) == "expression_statement")
            .map_or(assertion.end_byte(), |s| s.end_byte());
        if kind(body) == "block" && work_after(ctx, body, stmt_end) {
            return;
        }
        let at = assertion.field(Field::Name).unwrap_or(assertion);
        ctx.emit(
            at,
            "This assertion runs on another thread: its failure never reaches the test",
        );
    }
}

impl<'t> JavaRule<'t> for AssertionInThread {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["object_creation_expression", "method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["execute", "submit", "runAsync", "supplyAsync"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !ctx.is_test() {
            return;
        }
        if kind(node) == "object_creation_expression" {
            let Some(ty) = node.field(Field::Type) else {
                return;
            };
            if !matches!(ctx.text(ty), "Thread" | "java.lang.Thread") {
                return;
            }
            if !self.open(ctx) {
                return;
            }
            if ctx.resolve_type_name(ctx.text(ty), node).as_deref() != Some("java.lang.Thread") {
                return;
            }
            if syntax::type_body(node).is_some() {
                self.check_task(ctx, node);
                return;
            }
            for a in util::args(node) {
                self.check_task(ctx, a);
            }
            return;
        }
        let Some(name) = ctx.call_name(node) else {
            return;
        };
        if !self.open(ctx) {
            return;
        }
        let discarded = ctx
            .parent(node)
            .is_some_and(|p| kind(p) == "expression_statement");
        let proven = match name {
            "execute" => ctx.is_call_to(node, "java.util.concurrent.Executor", "execute"),
            "submit" => {
                discarded && ctx.is_call_to(node, "java.util.concurrent.ExecutorService", "submit")
            }
            "runAsync" | "supplyAsync" => {
                discarded && ctx.is_call_to(node, "java.util.concurrent.CompletableFuture", name)
            }
            _ => false,
        };
        if !proven {
            return;
        }
        if let Some(task) = util::arg(node, 0) {
            self.check_task(ctx, task);
        }
    }
}

/// Whether a catch clause hands the caught error to a collector the test checks later
/// (`errors.add(e)`, `failure.set(e)`, `failure = e`): an assertion after that in the handler
/// is redundant, the failure already reaches the test.
fn handler_collects_error<'t>(ctx: &Ctx<'t>, clause: Node<'t>) -> bool {
    let Some(var) = syntax::child_of_kind(clause, "catch_formal_parameter")
        .and_then(|p| p.field(Field::Name))
        .map(|n| ctx.text(n))
    else {
        return false;
    };
    let Some(handler) = clause.field(Field::Body) else {
        return false;
    };
    let is_var = |e: Node<'t>| {
        let e = syntax::strip_parens(e);
        kind(e) == "identifier" && ctx.text(e) == var
    };
    util::any_in_scope(handler, |n| match kind(n) {
        "assignment_expression" => {
            syntax::operator_str(n) == "=" && n.field(Field::Right).is_some_and(is_var)
        }
        "method_invocation" => {
            let name = ctx.call_name(n).unwrap_or("");
            let stores = ["add", "offer", "put", "push", "record", "collect"]
                .iter()
                .any(|p| name.starts_with(p))
                || matches!(
                    name,
                    "set" | "lazySet" | "compareAndSet" | "complete" | "completeExceptionally"
                );
            let on_logger = n
                .field(Field::Object)
                .is_some_and(|o| ctx.text(o).to_ascii_lowercase().contains("log"));
            stores && !on_logger && util::args(n).any(is_var)
        }
        _ => false,
    })
}
