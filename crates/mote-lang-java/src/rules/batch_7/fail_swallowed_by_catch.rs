//! `java/fail-swallowed-by-catch` — `try { …; fail(); } catch (Throwable t) { }`: the
//! catch swallows the `AssertionError` thrown by `fail()`.

use super::util::{self, Walk};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Stable,
    tags: &["tests", "exceptions"],
    remediation_minutes: 5,
    description: r#"Detects `fail(..)`, or a test assertion (`assertEquals`, `assertThat`, …), inside a
`try` block whose first handler able to catch an `AssertionError` — `catch (Throwable ..)`,
`catch (Error ..)`, `catch (AssertionError ..)` — neither rethrows nor asserts anything.

**Why it matters.** `fail()` and failing assertions work by throwing `AssertionError`. The
"expected exception" idiom `try { call(); fail(); } catch (Throwable expected) { }` therefore
catches its own failure: when `call()` does *not* throw, `fail()` throws, the handler swallows
it, and the test passes. Likewise an `assertEquals` inside `try { … } catch (Throwable t) {
log(t); }` can never fail the test. The test cannot detect the regression it was written for.

**How to fix.** Catch the specific exception the code should throw, or use
`assertThrows` / `assertThatThrownBy`; keep assertions outside a catch-all.

```java
// Noncompliant
try {
    parser.parse("");
    fail("expected a parse error");
} catch (Throwable expected) {
}

// Compliant
assertThrows(ParseException.class, () -> parser.parse(""));
```

A handler that rethrows, throws something else, or makes an assertion about the caught
exception (which would fail on the `AssertionError`) is not reported, nor is a `fail()` that
an earlier, narrower `catch (AssertionError ..)` handles, nor one whose handler hands the
caught error on for a later check (`errors.add(e)`, `failure = e`). For assertions other than
`fail()`, a handler that does anything beyond logging or printing the error (adds it to a list,
counts it, stores it for a later check, calls other code) is not reported either, nor is a
`try` inside a loop (a retry loop that swallows failed attempts on purpose and fails after the
loop), nor a handler that ends with `return`/`break`/`continue` when the failure still has an
outcome: the handler returns a value the `try` block never returns (`return true` after the
assertion, `return false` in the handler: a polling check handed to a wait-until helper that
fails the test on time-out), or the code after the `try` fails the test (`catch
(AssertionError e) { return; } fail(..);` — the assertion is the code under test, expected to
throw)."#,
    ..RuleMeta::new(
        "java/fail-swallowed-by-catch",
        "java",
        "Test failure swallowed by its own catch",
    )
};

#[derive(Default)]
pub struct FailSwallowedByCatch {
    fails: util::Gate,
}

/// Whether a catch clause's types include one that catches `AssertionError`.
fn catches_assertion_error<'t>(ctx: &Ctx<'t>, clause: Node<'t>) -> bool {
    let Some(param) = syntax::child_of_kind(clause, "catch_formal_parameter") else {
        return false;
    };
    let Some(types) = syntax::child_of_kind(param, "catch_type") else {
        return false;
    };
    named_children(types).any(|t| {
        matches!(
            util::last_segment(ctx.text(t)),
            "Throwable" | "Error" | "AssertionError" | "AssertionFailedError"
        )
    })
}

/// Whether a handler body rethrows or asserts (then the swallowed failure still surfaces).
fn handler_reacts<'t>(ctx: &Ctx<'t>, body: Node<'t>) -> bool {
    util::any_in_scope(body, |n| match kind(n) {
        "throw_statement" | "assert_statement" => true,
        "method_invocation" => ctx.call_name(n).is_some_and(|name| {
            name.starts_with("assert")
                || name.starts_with("fail")
                || name.starts_with("verify")
                || name.starts_with("expect")
                || name == "rethrow"
                || name == "propagate"
        }),
        _ => false,
    })
}

impl<'t> JavaRule<'t> for FailSwallowedByCatch {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["try_statement", "try_with_resources_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !self.fails.open_any(ctx, &["fail", "assert"]) {
            return;
        }
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        let body_text = ctx.text(body);
        if !body_text.contains("fail") && !body_text.contains("assert") {
            return;
        }
        // The first handler that can catch the AssertionError decides.
        let Some(clause) = named_children(node)
            .filter(|c| kind(*c) == "catch_clause")
            .find(|c| catches_assertion_error(ctx, *c))
        else {
            return;
        };
        let Some(handler) = clause.field(Field::Body) else {
            return;
        };
        if handler_reacts(ctx, handler) {
            return;
        }
        // A `try` nested in the body handles its own failures: don't look inside.
        let mut fail = None;
        let mut assertion = None;
        util::visit(body, |n| {
            if util::is_nested_scope(n)
                || matches!(kind(n), "try_statement" | "try_with_resources_statement")
            {
                return Walk::Skip;
            }
            if kind(n) != "method_invocation" {
                return Walk::Continue;
            }
            let Some(name) = ctx.call_name(n) else {
                return Walk::Continue;
            };
            let library_receiver = n.field(Field::Object).is_none_or(|o| {
                matches!(
                    util::last_segment(ctx.text(o)),
                    "Assert" | "Assertions" | "TestCase" | "Fail" | "MatcherAssert"
                )
            });
            if matches!(name, "fail" | "failBecauseExceptionWasNotThrown") && library_receiver {
                fail = Some(n);
                return Walk::Stop;
            }
            if assertion.is_none() && name.starts_with("assert") && library_receiver {
                assertion = Some(n);
            }
            Walk::Continue
        });
        if let Some(fail) = fail {
            // `catch (Throwable e) { errors.add(e); }` hands the failure on for a later check.
            if stores_caught_error(ctx, clause, handler) {
                return;
            }
            ctx.emit(
                fail,
                "Catch the specific expected exception (or use assertThrows): this handler also catches the AssertionError thrown by fail(), so the test cannot fail here",
            )
            .secondary(clause, "this handler swallows the failure");
            return;
        }
        // An assertion failure is an `AssertionError` as well; a handler that keeps the caught
        // error (collects it, stores it for a later check) is not swallowing it.
        let Some(assertion) = assertion else {
            return;
        };
        if keeps_caught_error(ctx, clause, handler) {
            return;
        }
        // Retry loops (`for (…) { try { assertX(…); return; } catch (AssertionError e) { } }`
        // followed by a final `fail()`) swallow failed attempts on purpose.
        let in_loop = ctx
            .ancestors(node)
            .take_while(|a| !syntax::is_function_like(*a))
            .any(|a| {
                matches!(
                    kind(a),
                    "for_statement" | "enhanced_for_statement" | "while_statement" | "do_statement"
                )
            });
        if in_loop {
            return;
        }
        if failure_turned_into_outcome(ctx, node, body, handler) {
            return;
        }
        let name = ctx.call_name(assertion).unwrap_or("assert");
        ctx.emit(
            assertion,
            format!("Catch the specific expected exception instead: this handler also catches the AssertionError thrown by `{name}`, so the assertion can never fail the test"),
        )
        .secondary(clause, "this handler swallows the failure");
    }
}

/// Whether the handler does anything but log or print: records the failure (assignment,
/// counter, `failures.add(e)`), creates objects, or calls code we can't see. Only a handler
/// that merely logs, prints or ignores the error swallows it for sure.
fn keeps_caught_error<'t>(ctx: &Ctx<'t>, clause: Node<'t>, handler: Node<'t>) -> bool {
    let var = syntax::child_of_kind(clause, "catch_formal_parameter")
        .and_then(|p| p.field(Field::Name))
        .map(|n| ctx.text(n));
    util::any_in_scope(handler, |n| match kind(n) {
        "assignment_expression" | "update_expression" | "object_creation_expression" => true,
        "method_invocation" => {
            let name = ctx.call_name(n).unwrap_or("");
            let recv = n.field(Field::Object).map(|o| ctx.text(o)).unwrap_or("");
            let on_error = var.is_some_and(|v| recv == v);
            let logs = matches!(
                name,
                "trace"
                    | "debug"
                    | "info"
                    | "warn"
                    | "warning"
                    | "error"
                    | "log"
                    | "println"
                    | "print"
                    | "printf"
            ) && (recv.to_ascii_lowercase().contains("log")
                || recv == "System.out"
                || recv == "System.err");
            !(on_error || logs)
        }
        _ => false,
    })
}

/// The name of the caught exception variable of a catch clause.
fn caught_var<'t>(ctx: &Ctx<'t>, clause: Node<'t>) -> Option<&'t str> {
    syntax::child_of_kind(clause, "catch_formal_parameter")
        .and_then(|p| p.field(Field::Name))
        .map(|n| ctx.text(n))
}

/// Whether the handler hands the caught error itself on for a later check: stores it
/// (`failure = e`) or passes it to a collecting call (`errors.add(e)`, `ref.set(e)`,
/// `future.completeExceptionally(e)`). Logging calls (`log.error("..", e)`) don't count.
fn stores_caught_error<'t>(ctx: &Ctx<'t>, clause: Node<'t>, handler: Node<'t>) -> bool {
    let Some(var) = caught_var(ctx, clause) else {
        return false;
    };
    let is_var = |e: Node<'t>| {
        let e = syntax::strip_parens(e);
        let e = if kind(e) == "cast_expression" {
            e.field(Field::Value).map_or(e, syntax::strip_parens)
        } else {
            e
        };
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
                    "set"
                        | "lazySet"
                        | "compareAndSet"
                        | "getAndSet"
                        | "complete"
                        | "completeExceptionally"
                        | "setException"
                        | "accept"
                );
            let on_logger = n
                .field(Field::Object)
                .is_some_and(|o| ctx.text(o).to_ascii_lowercase().contains("log"));
            stores && !on_logger && util::args(n).any(is_var)
        }
        _ => false,
    })
}

/// Whether the handler turns the failure into an outcome instead of dropping it: it ends with
/// a jump (`return`, `break`, `continue`) and either the code that runs when the `try` block
/// completes fails the test (`try { assertX(..); } catch (AssertionError e) { return; }
/// fail("expected a failure");` — the assertion is the code under test), or it returns a
/// value the `try` block never returns (`try { assertX(..); return true; } catch
/// (AssertionError e) { return false; }`, a polling check handed to a wait-until helper).
fn failure_turned_into_outcome<'t>(
    ctx: &Ctx<'t>,
    try_stmt: Node<'t>,
    body: Node<'t>,
    handler: Node<'t>,
) -> bool {
    let Some(last) = named_children(handler)
        .filter(|s| !syntax::is_comment(*s))
        .last()
    else {
        return false;
    };
    if !matches!(
        kind(last),
        "return_statement" | "break_statement" | "continue_statement"
    ) {
        return false;
    }
    // A distinct result: every `return` of the try block returns something else.
    if kind(last) == "return_statement" {
        if let Some(value) = syntax::first_named_child(last) {
            let caught_result = ctx.text(value);
            let mut try_results = 0usize;
            let mut same = false;
            util::visit(body, |n| {
                if util::is_nested_scope(n) {
                    return Walk::Skip;
                }
                if kind(n) == "return_statement" {
                    match syntax::first_named_child(n) {
                        Some(v) => {
                            try_results += 1;
                            same |= ctx.text(v) == caught_result;
                        }
                        None => same = true,
                    }
                    return Walk::Skip;
                }
                Walk::Continue
            });
            if try_results > 0 && !same {
                return true;
            }
        }
    }
    // The statements that follow the `try` (in it and in the enclosing blocks) fail the test.
    let mut cur = try_stmt;
    for a in ctx.ancestors(try_stmt) {
        if syntax::is_function_like(a) || util::is_nested_scope(a) {
            break;
        }
        if kind(a) == "block" {
            let fails = named_children(a)
                .skip_while(|s| s.id() != cur.id())
                .skip(1)
                .any(|s| fails_test(ctx, s));
            if fails {
                return true;
            }
        }
        cur = a;
    }
    false
}

/// `fail(..)` / `Assertions.fail(..)` / `failBecauseExceptionWasNotThrown(..)` as a statement,
/// or a `throw`.
fn fails_test<'t>(ctx: &Ctx<'t>, stmt: Node<'t>) -> bool {
    match kind(stmt) {
        "throw_statement" => true,
        "expression_statement" => syntax::first_named_child(stmt).is_some_and(|e| {
            kind(e) == "method_invocation"
                && matches!(
                    ctx.call_name(e),
                    Some("fail" | "failBecauseExceptionWasNotThrown")
                )
        }),
        _ => false,
    }
}
