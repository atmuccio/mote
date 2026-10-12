//! `java/exception-logged-without-trace` — a catch block that logs only the message of the
//! caught exception, losing its stack trace.

use super::util::{self, LogLevel, Walk};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["error-handling", "logging"],
    remediation_minutes: 2,
    description: r#"Detects a catch block that reports the caught exception at warning or error
level through a logger, but hands the logger only its text — `e.getMessage()`,
`e.getLocalizedMessage()`, `e.toString()` or `"…" + e` — and does nothing else with it: the
exception is not passed to the logger, not rethrown, not wrapped, not stored.

**Why it matters.** The handler is the last place that has the exception. Once only its
message is written, the stack trace and the chain of causes are gone for good: the log says
*that* something failed (sometimes only `null`, since many exceptions have no message) but not
*where*, and the root cause buried in `getCause()` is never seen. At warning or error level
the failure is by definition something someone will have to investigate.

**How to fix.** Pass the exception itself as the last argument of the logging call; every
logging API prints its stack trace and causes then.

```java
// Noncompliant
try {
    publisher.send(event);
} catch (IOException e) {
    log.error("could not publish event: " + e.getMessage());
}

// Compliant
try {
    publisher.send(event);
} catch (IOException e) {
    log.error("could not publish event {}", event.id(), e);
}
```

Loggers are recognized by their declared type (SLF4J, Log4j, Commons Logging, JBoss Logging,
`java.util.logging`) or by a conventional name in a file importing a logging API. Only
handlers after which the work is given up are considered: those in `void` methods,
constructors and initializers that assign nothing. Not reported: info, debug and trace
output; handlers that recover with a fallback value (a handler in a method returning a value
falls back to one, here or after the `try`; so does one that assigns a variable), since the
failure was expected and dealt with; handlers with an explanatory comment in their block (it
documents the failure as expected, and usually the fallback that follows: `// treat the same
as end of file`; tool markers and task tags written in capitals such as `// NOPMD` or `// TODO …`, and
commented-out code do not count);
handlers that throw, or use the exception in any way
other than for its text (the trace may be kept elsewhere); handlers inside lambdas; handlers
of `InterruptedException`, whose trace only shows where the thread was waiting; and test
code."#,
    ..RuleMeta::new(
        "java/exception-logged-without-trace",
        "java",
        "Exception logged without its stack trace",
    )
};

pub struct ExceptionLoggedWithoutTrace;

/// Methods whose result is only the text of their receiver.
fn is_text_method(name: &str) -> bool {
    matches!(name, "getMessage" | "getLocalizedMessage" | "toString")
}

/// Whether a comment explains something in prose: not a marker for a tool or a task tag, which
/// start with a word in capitals (`//NOPMD`, `// CHECKSTYLE:OFF`, `// TODO …`, `// XXX: …`) or
/// with `@` / `$` (`// @formatter:off`, `// $NON-NLS-1$`), and not commented-out code
/// (`// e.printStackTrace();`).
fn is_explanation(comment: &str) -> bool {
    let body = comment
        .trim_start_matches('/')
        .trim_start_matches('*')
        .trim_end_matches('/')
        .trim_end_matches('*')
        .trim();
    if body.is_empty() || body.starts_with(['@', '$']) {
        return false;
    }
    let first: &str = body
        .split(|c: char| !c.is_ascii_alphanumeric())
        .next()
        .unwrap_or("");
    let tag = first.len() >= 3
        && first
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit());
    let code_like = body.ends_with(';') || body.ends_with('{') || body.ends_with('}');
    !tag && !code_like
}

/// Whether the use `ident` of the caught exception (with parent `parent`) only reads its text.
fn is_text_use<'t>(ctx: &Ctx<'t>, ident: Node<'t>, parent: Node<'t>) -> bool {
    match kind(parent) {
        "method_invocation" => {
            parent
                .field(Field::Object)
                .is_some_and(|o| o.id() == ident.id())
                && ctx.call_name(parent).is_some_and(is_text_method)
                && util::arg_count(parent) == 0
        }
        // `"failed: " + e` calls `toString()`.
        "binary_expression" => {
            syntax::operator_str(parent) == "+"
                && ctx.type_of(parent).is_some_and(|t| t.is_string())
        }
        _ => false,
    }
}

impl<'t> JavaRule<'t> for ExceptionLoggedWithoutTrace {
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
        if ctx.is_test() {
            return;
        }
        let Some(name) = util::catch_param_name(node, ctx) else {
            return;
        };
        // The trace of an interruption only shows where the thread was waiting.
        if util::catch_types(node)
            .any(|t| util::last_segment(ctx.text(util::type_base(t))) == "InterruptedException")
        {
            return;
        }
        let text = ctx.text(body);
        if !text.contains(name) {
            return;
        }
        // A handler that explains itself in a comment documents an expected failure and its
        // fallback (`// treat the same as end of file`): the message is all that is wanted.
        if syntax::children(body).any(|c| {
            matches!(kind(c), "line_comment" | "block_comment") && is_explanation(ctx.text(c))
        }) {
            return;
        }
        // Only handlers after which the work is abandoned: in a method returning a value the
        // handler necessarily falls back to some value (returned here or after the `try`).
        let abandons = match ctx.enclosing_function(node) {
            Some(f) => match kind(f) {
                "method_declaration" => {
                    f.field(Field::Type).is_some_and(|t| kind(t) == "void_type")
                }
                "constructor_declaration" | "static_initializer" | "block" => true,
                _ => false,
            },
            None => false,
        };
        if !abandons {
            return;
        }
        // Every use of the exception must read only its text; the handler must neither throw
        // nor return a value.
        let mut bail = false;
        let mut text_uses: Vec<Node<'t>> = Vec::new();
        // (node, parent) pairs are not available from `visit`; track the parent by position.
        let mut stack: Vec<(Node<'t>, Node<'t>)> =
            named_children(body).map(|c| (c, body)).collect();
        let mut steps = 0u32;
        while let Some((n, parent)) = stack.pop() {
            steps += 1;
            if steps > 20_000 {
                return;
            }
            match kind(n) {
                // A rethrow, or a fallback value (assigned or returned).
                "throw_statement" | "assignment_expression" => bail = true,
                "return_statement" if syntax::first_named_child(n).is_some() => bail = true,
                "lambda_expression" | "class_body" => {
                    if util::mentions(ctx, n, name) {
                        bail = true;
                    }
                    continue;
                }
                "identifier" => {
                    if ctx.text(n) == name {
                        let is_member_name = kind(parent) == "field_access"
                            && parent.field(Field::Field).is_some_and(|f| f.id() == n.id());
                        if is_member_name {
                            continue;
                        }
                        if is_text_use(ctx, n, parent) {
                            text_uses.push(n);
                        } else {
                            bail = true;
                        }
                    }
                    continue;
                }
                _ => {}
            }
            if bail {
                return;
            }
            for c in named_children(n) {
                stack.push((c, n));
            }
        }
        if bail || text_uses.is_empty() {
            return;
        }
        // A warning- or error-level logging call whose arguments read the exception's text.
        let mut found: Option<(Node<'t>, Node<'t>)> = None;
        util::visit(body, |n| {
            if util::is_other_context(n) {
                return Walk::Skip;
            }
            if kind(n) != "method_invocation" {
                return Walk::Continue;
            }
            let Some(args) = n.field(Field::Arguments) else {
                return Walk::Continue;
            };
            let Some(use_) = text_uses
                .iter()
                .find(|u| u.start_byte() >= args.start_byte() && u.end_byte() <= args.end_byte())
            else {
                return Walk::Continue;
            };
            if util::log_call_level(ctx, n).is_some_and(|l| l >= LogLevel::Warn) {
                found = Some((n, *use_));
                return Walk::Stop;
            }
            Walk::Continue
        });
        let Some((_call, use_)) = found else {
            return;
        };
        let at = ctx
            .parent(use_)
            .filter(|p| kind(*p) == "method_invocation")
            .unwrap_or(use_);
        ctx.emit(
            at,
            format!(
                "Pass the exception `{name}` itself to the logger: logging only its text loses \
                 the stack trace and the causes"
            ),
        );
    }
}
