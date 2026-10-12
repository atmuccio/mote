//! `java/catch-programming-error` — catching `NullPointerException` or
//! `IllegalMonitorStateException`, exceptions that signal a bug in the calling code.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["error-handling", "suspicious"],
    remediation_minutes: 10,
    description: r#"Detects catch clauses for `NullPointerException` or
`IllegalMonitorStateException` in production code.

**Why it matters.** Both exceptions are thrown when the program itself is wrong: a reference
that should have been checked was `null`, or `wait`/`notify`/`unlock` was called without
holding the lock. Catching them treats the symptom: the bug stays, the handler also swallows
every *other* occurrence of the same exception in the protected code (including ones nobody
anticipated), and the state the code was manipulating may be left half updated. A null check
or the correct lock says what the code expects; a handler hides it.

**How to fix.** Find out which reference can be `null` and test it (or make sure it can't be),
or call the monitor methods while holding the lock; then remove the handler.

```java
// Noncompliant
try {
    return customer.getAddress().getCity();
} catch (NullPointerException e) {
    return "";
}

// Compliant
Address address = customer.getAddress();
return address == null ? "" : address.getCity();
```

Not reported: test code (tests catch these on purpose to check that they are thrown), a catch
parameter named `expected`, handlers with a comment in their block (a documented workaround,
typically for a failure inside a library the author cannot fix), handlers that throw the caught exception on (always, to keep a
later `catch (Exception e)` from wrapping it, or when it is not the one they expect), and
throwing these exceptions — `Objects.requireNonNull` and
explicit `throw new NullPointerException(…)` for a null argument are the JDK's own
convention. Wrapping the caught exception as the cause of another one
(`throw new IllegalStateException("…", e)`) does not count as throwing it on: the handler
still turns the bug into a different failure that callers may catch and handle, so it is
reported."#,
    ..RuleMeta::new(
        "java/catch-programming-error",
        "java",
        "Programming error caught instead of fixed",
    )
};

pub struct CatchProgrammingError;

const CAUGHT: &[(&str, &str)] = &[
    ("NullPointerException", "java.lang.NullPointerException"),
    (
        "IllegalMonitorStateException",
        "java.lang.IllegalMonitorStateException",
    ),
];

impl<'t> JavaRule<'t> for CatchProgrammingError {
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
        let name = util::catch_param_name(node, ctx);
        if name == Some("expected") {
            return;
        }
        // A handler that explains itself in a comment is a documented workaround (typically
        // for a failure inside a library the author cannot fix).
        if node
            .field(Field::Body)
            .is_some_and(syntax::block_has_comment)
        {
            return;
        }
        // A handler that throws the exception on (always, or when it is not the one it
        // expects) lets the error surface: `catch (NullPointerException e) { throw e; }` keeps
        // a following `catch (Exception e)` from wrapping it.
        if let (Some(name), Some(body)) = (name, node.field(Field::Body)) {
            if ctx.text(body).contains("throw")
                && util::visit(body, |n| {
                    if kind(n) == "throw_statement"
                        && syntax::first_named_child(n)
                            .map(syntax::strip_parens)
                            .is_some_and(|t| kind(t) == "identifier" && ctx.text(t) == name)
                    {
                        util::Walk::Stop
                    } else {
                        util::Walk::Continue
                    }
                })
            {
                return;
            }
        }
        for t in util::catch_types(node) {
            let simple = util::last_segment(ctx.text(util::type_base(t)));
            let Some(&(simple, fqn)) = CAUGHT.iter().find(|c| c.0 == simple) else {
                continue;
            };
            if !util::is_java_lang_type(ctx, t, fqn) {
                continue;
            }
            let advice = if simple == "NullPointerException" {
                "check the reference that can be null instead"
            } else {
                "hold the lock when calling the monitor methods instead"
            };
            ctx.emit(t, format!("Don't catch `{simple}`: {advice}"));
        }
    }
}
