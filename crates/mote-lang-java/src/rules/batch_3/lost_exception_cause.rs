//! `java/lost-exception-cause` — a new exception thrown from a catch block without the cause.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Medium,
    tags: &["error-handling"],
    remediation_minutes: 5,
    description: r#"Detects exceptions thrown from a `catch (E e)` block — `throw new X(…)` or an
exception built by a helper, `throw invalidInput(…)` — that do not receive `e` itself: either
`e` is not mentioned at all, or only its text is (`e.getMessage()`, `e.toString()`,
`"failed: " + e`).

**Why it matters.** The caught exception carries the stack trace and message of the real
failure, and the chain of its own causes. Throwing a fresh exception without chaining it
discards that information: the logs show where the translation happened, not what went wrong.
Copying the message keeps a line of text but still loses the stack trace and the causes.

**How to fix.** Pass the caught exception as the cause (most exception types have a
`(String, Throwable)` constructor), or call `initCause(e)` / `addSuppressed(e)` when the
constructor doesn't accept one.

```java
// Noncompliant
try {
    return Integer.parseInt(text);
} catch (NumberFormatException e) {
    throw new ConfigException("invalid port: " + e.getMessage());
}

// Compliant
try {
    return Integer.parseInt(text);
} catch (NumberFormatException e) {
    throw new ConfigException("invalid port: " + text, e);
}
```

Not reported: catch blocks that call `initCause` or `addSuppressed`, caught variables named
`ignored`/`ignore`/`expected`, throws inside a nested `try` that has its own handlers, throws
inside lambdas or anonymous classes declared in the catch block, rethrowing a variable
(`throw e`, `throw cause`), and exceptions that receive the caught variable in any other way
than for its text — as an argument (`new X(e)`, `wrap(e)`, `ExceptionUtils.getStackTrace(e)`),
through `e.getCause()`, through the variable of an enclosing catch clause, or through a local
derived from them (`Throwable root = e.getCause()`). A helper-built exception is also not
reported when the catch block stores the caught exception somewhere (`this.failure = e`)."#,
    ..RuleMeta::new(
        "java/lost-exception-cause",
        "java",
        "Exception cause lost when rethrowing",
    )
};

pub struct LostExceptionCause;

/// How an expression refers to the caught exception.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Refs {
    /// Not at all.
    None,
    /// Only through its text: `e.getMessage()`, `"…" + e`, a `String` derived from it.
    TextOnly,
    /// In a way that may pass the exception on (argument, `e.getCause()`, …).
    Chained,
}

impl Refs {
    fn join(self, other: Refs) -> Refs {
        match (self, other) {
            (Refs::Chained, _) | (_, Refs::Chained) => Refs::Chained,
            (Refs::TextOnly, _) | (_, Refs::TextOnly) => Refs::TextOnly,
            _ => Refs::None,
        }
    }
}

/// The exception variables in scope at the `throw`: names that carry the exception and names
/// of strings derived from its text.
struct Names<'t> {
    chained: Vec<&'t str>,
    text: Vec<&'t str>,
    /// The caught exception was stored in a field or array (`this.failure = e`).
    stored: bool,
}

impl<'t> JavaRule<'t> for LostExceptionCause {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["throw_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(thrown) = syntax::first_named_child(node).map(syntax::strip_parens) else {
            return;
        };
        let is_new = match kind(thrown) {
            "object_creation_expression" => true,
            "method_invocation" => false,
            _ => return,
        };
        let Some(catch) = enclosing_catch(node, ctx) else {
            return;
        };
        let Some(name) = util::catch_param_name(catch, ctx) else {
            return;
        };
        if matches!(name, "ignored" | "ignore" | "expected") {
            return;
        }
        // Exceptions the new one may legitimately chain: this catch's parameter, those of
        // enclosing catch clauses, and locals derived from them (`Throwable c = e.getCause()`).
        let mut names = Names {
            chained: vec![name],
            text: Vec::new(),
            stored: false,
        };
        let mut outermost = catch;
        let mut prev = catch;
        for a in ctx.ancestors(catch) {
            match kind(a) {
                "catch_clause" => {
                    if a.field(Field::Body).is_some_and(|b| b.id() == prev.id()) {
                        if let Some(n) = util::catch_param_name(a, ctx) {
                            names.chained.push(n);
                        }
                        outermost = a;
                    }
                }
                k if is_boundary(k) => break,
                _ => {}
            }
            prev = a;
        }
        let Some(outer_body) = outermost.field(Field::Body) else {
            return;
        };
        collect_derived(outer_body, node.start_byte(), ctx, &mut names);
        let refs = refs_in(thrown, &names, ctx);
        if refs == Refs::Chained || (!is_new && names.stored) {
            return;
        }
        let Some(body) = catch.field(Field::Body) else {
            return;
        };
        let chained = util::any_descendant(
            body,
            |_| false,
            |n| {
                kind(n) == "method_invocation"
                    && util::name_field(n)
                        .is_some_and(|m| matches!(ctx.text(m), "initCause" | "addSuppressed"))
            },
        );
        if chained {
            return;
        }
        if is_new {
            let end = thrown.field(Field::Type).unwrap_or(thrown);
            let msg = if refs == Refs::TextOnly {
                format!("Pass the caught exception `{name}` itself as the cause of this new exception, not only its message")
            } else {
                format!("Pass the caught exception `{name}` as the cause of this new exception")
            };
            ctx.emit_range(Span::cover(thrown, end), msg);
        } else {
            let Some(method) = util::name_field(thrown) else {
                return;
            };
            ctx.emit(
                method,
                format!(
                    "Chain the caught exception `{name}` as the cause of the exception built here"
                ),
            );
        }
    }
}

fn is_boundary(k: &str) -> bool {
    matches!(
        k,
        "lambda_expression"
            | "class_body"
            | "interface_body"
            | "enum_body"
            | "method_declaration"
            | "constructor_declaration"
            | "compact_constructor_declaration"
            | "static_initializer"
    )
}

/// Methods whose result is only the text (or class) of their receiver.
fn is_text_method(name: &str) -> bool {
    matches!(
        name,
        "getMessage" | "getLocalizedMessage" | "toString" | "getClass"
    )
}

/// How the identifier `ident` (one of the exception names, with parent `parent`) is used.
fn classify_use<'t>(ident: Node<'t>, parent: Option<Node<'t>>, ctx: &Ctx<'t>) -> Refs {
    let Some(p) = parent else {
        return Refs::Chained;
    };
    match kind(p) {
        "method_invocation" => {
            let is_receiver = p.field(Field::Object).is_some_and(|o| o.id() == ident.id());
            let text_method = util::name_field(p).is_some_and(|m| is_text_method(ctx.text(m)));
            if is_receiver && text_method {
                Refs::TextOnly
            } else {
                Refs::Chained
            }
        }
        // `"failed: " + e` — string concatenation calls `toString()`.
        "binary_expression"
            if syntax::operator_str(p) == "+" && ctx.type_of(p).is_some_and(|t| t.is_string()) =>
        {
            Refs::TextOnly
        }
        _ => Refs::Chained,
    }
}

/// How `root` refers to the exception names (lambdas and class bodies inside are skipped: code
/// there runs elsewhere and cannot be judged here — they count as passing the exception on).
fn refs_in<'t>(root: Node<'t>, names: &Names<'t>, ctx: &Ctx<'t>) -> Refs {
    let mut acc = Refs::None;
    // (node, parent)
    let mut stack: Vec<(Node<'t>, Option<Node<'t>>)> = vec![(root, None)];
    let mut steps = 0u32;
    while let Some((n, parent)) = stack.pop() {
        steps += 1;
        if steps > 10_000 {
            return Refs::Chained;
        }
        if kind(n) == "identifier" {
            let t = ctx.text(n);
            if names.chained.contains(&t) {
                // A field access `x.e` names another variable.
                let is_field_name = parent.is_some_and(|p| {
                    kind(p) == "field_access"
                        && p.field(Field::Field).is_some_and(|f| f.id() == n.id())
                });
                if !is_field_name {
                    acc = acc.join(classify_use(n, parent, ctx));
                }
            } else if names.text.contains(&t) {
                acc = acc.join(Refs::TextOnly);
            }
            if acc == Refs::Chained {
                return acc;
            }
            continue;
        }
        if util::is_other_context(n) && parent.is_some() {
            // A lambda or anonymous class mentioning the exception may do anything with it.
            let inner = refs_in(n, names, ctx);
            if inner != Refs::None {
                return Refs::Chained;
            }
            continue;
        }
        let mut cur = n.walk();
        for ch in n.named_children(&mut cur) {
            stack.push((ch, Some(n)));
        }
    }
    acc
}

/// Adds to `names` the variables assigned from an expression that mentions one of them, in
/// source order, up to byte offset `limit` (the `throw`): strings built from the exception's
/// text go to `names.text`, anything else to `names.chained`. Also notes whether the exception
/// is stored in a field or array element.
fn collect_derived<'t>(body: Node<'t>, limit: usize, ctx: &Ctx<'t>, names: &mut Names<'t>) {
    util::visit(body, |n| {
        if n.start_byte() >= limit {
            return util::Walk::Stop;
        }
        let (target, value) = match kind(n) {
            "variable_declarator" => (util::name_field(n), n.field(Field::Value)),
            "instanceof_expression" => (util::name_field(n), n.field(Field::Left)),
            "assignment_expression" => {
                let right = n.field(Field::Right);
                match n.field(Field::Left) {
                    Some(l) if kind(l) == "identifier" => (Some(l), right),
                    _ => {
                        if right.is_some_and(|r| refs_in(r, names, ctx) == Refs::Chained) {
                            names.stored = true;
                        }
                        (None, None)
                    }
                }
            }
            _ => (None, None),
        };
        if let (Some(target), Some(value)) = (target, value) {
            let refs = refs_in(value, names, ctx);
            let t = ctx.text(target);
            let list = match refs {
                Refs::None => None,
                Refs::TextOnly if kind(n) == "variable_declarator" && declares_string(n, ctx) => {
                    Some(&mut names.text)
                }
                _ => Some(&mut names.chained),
            };
            if let Some(list) = list {
                if !list.contains(&t) {
                    list.push(t);
                }
            }
        }
        util::Walk::Continue
    });
}

/// Whether a `variable_declarator` belongs to a declaration of type `String`.
fn declares_string<'t>(declarator: Node<'t>, ctx: &Ctx<'t>) -> bool {
    ctx.parent(declarator)
        .and_then(|d| d.field(Field::Type))
        .is_some_and(|t| matches!(ctx.text(t), "String" | "java.lang.String"))
}

/// The catch clause whose body contains `node` (innermost), unless a function or class
/// boundary, or a nested `try` with handlers, comes first.
fn enclosing_catch<'t>(node: Node<'t>, ctx: &Ctx<'t>) -> Option<Node<'t>> {
    let mut prev = node;
    for a in ctx.ancestors(node) {
        match kind(a) {
            "catch_clause" => {
                let in_body = a.field(Field::Body).is_some_and(|b| b.id() == prev.id());
                return in_body.then_some(a);
            }
            "try_statement" | "try_with_resources_statement" => {
                let in_body = a.field(Field::Body).is_some_and(|b| b.id() == prev.id());
                if in_body && named_children(a).any(|c| kind(c) == "catch_clause") {
                    return None;
                }
            }
            k if is_boundary(k) => return None,
            _ => {}
        }
        prev = a;
    }
    None
}
