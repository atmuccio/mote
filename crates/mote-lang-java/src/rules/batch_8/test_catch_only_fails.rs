//! `java/test-catch-only-fails` — a test catches an exception only to call `fail()`.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["tests", "junit"],
    remediation_minutes: 5,
    description: r#"Detects `catch` blocks in test code whose only statement is a `fail(..)` call
of a test library (JUnit, AssertJ, TestNG).

**Why it matters.** A test fails anyway when an exception escapes it, and the report then
shows the exception with its full stack trace. Catching it just to call `fail()` replaces
that with a message (often `e.getMessage()`, sometimes nothing), so the place where the
error happened is lost, and the extra `try`/`catch` makes the test longer and hides what it
actually verifies.

**How to fix.** Let the exception propagate: declare it in the test method's `throws`
clause. To state explicitly that a call must not throw, JUnit 5 has `assertDoesNotThrow`.

```java
// Noncompliant
@Test
void parses() {
    try {
        parser.parse(input);
    } catch (IOException e) {
        fail("unexpected: " + e.getMessage());
    }
}

// Compliant
@Test
void parses() throws IOException {
    parser.parse(input);
}
```

Only test files are checked. Calls that pass the caught exception on as the cause
(`fail(message, e)`) keep its stack trace and are not reported. Catches of `Throwable` or of
an `Error` (`AssertionError`,
`StackOverflowError`, …), which usually add context to a failure on purpose, are not
reported, nor are catches inside lambdas, anonymous classes and overriding methods, where a checked
exception may not be rethrown, nor in methods used as a method reference in the file
(`addTest("dir", this::dirTest)`: the functional interface, usually declared elsewhere, may not
declare the exception either) and `private` helpers called only from such methods."#,
    ..RuleMeta::new(
        "java/test-catch-only-fails",
        "java",
        "Test catch block only calls fail()",
    )
};

#[derive(Default)]
pub struct TestCatchOnlyFails<'t> {
    /// Names used as method references anywhere in the file (`this::helpLicenses`).
    referenced: Vec<&'t str>,
    /// Candidate findings: the `fail` name node and the method holding the catch.
    parked: Vec<(Node<'t>, Node<'t>)>,
}

/// The method name of a method reference (`helpLicenses` in `this::helpLicenses`).
fn referenced_name<'t>(ctx: &Ctx<'t>, r: Node<'t>) -> Option<&'t str> {
    named_children(r)
        .last()
        .filter(|n| kind(*n) == "identifier")
        .map(|n| ctx.text(n))
}

/// Whether the `private` method `method` is called in this file, and every call sits in a method
/// that is used as a method reference: a helper one level below such a method, where the
/// exception would have nowhere to go either.
fn helper_of_referenced<'t>(ctx: &Ctx<'t>, method: Node<'t>, referenced: &[&str]) -> bool {
    if !syntax::has_modifier(method, "private") {
        return false;
    }
    let Some(name_node) = method.field(Field::Name) else {
        return false;
    };
    let name = ctx.text(name_node);
    // A missing name (broken code): the search below would never advance.
    if name.is_empty() {
        return false;
    }
    let src = ctx.source();
    let is_ident = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    let mut calls = 0;
    let mut from = 0;
    while let Some(start) = src.get(from..).and_then(|s| s.find(name)).map(|i| i + from) {
        let end = start + name.len();
        from = end;
        if start == name_node.start_byte()
            || src[..start].chars().next_back().is_some_and(is_ident)
            || src[end..].chars().next().is_some_and(is_ident)
        {
            continue;
        }
        let Some(ident) = ctx
            .root()
            .descendant_for_byte_range(start, end)
            .filter(|n| kind(*n) == "identifier")
        else {
            continue;
        };
        let Some(parent) = ctx.parent(ident) else {
            return false;
        };
        match kind(parent) {
            "method_invocation"
                if parent
                    .field(Field::Name)
                    .is_some_and(|n| n.id() == ident.id()) =>
            {
                let caller = ctx
                    .enclosing_method(parent)
                    .filter(|m| kind(*m) == "method_declaration")
                    .map(|m| syntax::name_of(m, src));
                if !caller.is_some_and(|c| c != name && referenced.contains(&c)) {
                    return false;
                }
                calls += 1;
            }
            "method_reference" => return false,
            _ => {}
        }
    }
    calls > 0
}

impl<'t> JavaRule<'t> for TestCatchOnlyFails<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["catch_clause", "method_reference"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !ctx.is_test() {
            return;
        }
        if kind(node) == "method_reference" {
            if let Some(name) = referenced_name(ctx, node) {
                if !self.referenced.contains(&name) {
                    self.referenced.push(name);
                }
            }
            return;
        }
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        let mut stmts = named_children(body);
        let (Some(only), None) = (stmts.next(), stmts.next()) else {
            return;
        };
        if kind(only) != "expression_statement" {
            return;
        }
        let Some(call) = syntax::first_named_child(only) else {
            return;
        };
        if kind(call) != "method_invocation" {
            return;
        }
        let Some(name) = call.field(Field::Name) else {
            return;
        };
        if ctx.text(name) != "fail" || !util::is_test_library_call(ctx, call, "fail") {
            return;
        }
        // Catching an assertion failure or an `Error` to fail with more context (a seed, a
        // short message instead of a huge stack overflow trace) is deliberate.
        let caught = named_children(node)
            .find(|c| kind(*c) == "catch_formal_parameter")
            .map(|p| ctx.text(p))
            .unwrap_or("");
        // `fail(msg, e)` keeps the original exception as the cause: nothing is lost.
        let var = named_children(node)
            .find(|c| kind(*c) == "catch_formal_parameter")
            .and_then(|p| p.field(Field::Name))
            .map(|n| ctx.text(n));
        if var
            .is_some_and(|v| util::args(call).any(|a| kind(a) == "identifier" && ctx.text(a) == v))
        {
            return;
        }
        if caught
            .split(|ch: char| !ch.is_alphanumeric() && ch != '_')
            .any(|w| {
                w == "Throwable"
                    || w.ends_with("Error")
                    || w == "AssertionFailedError"
                    || w == "ComparisonFailure"
            })
        {
            return;
        }
        // Inside a lambda or an anonymous/local class method, a checked exception may have
        // nowhere to go.
        let Some(func) = ctx.enclosing_function(node) else {
            return;
        };
        // An overriding method cannot widen its `throws` clause.
        if kind(func) != "method_declaration"
            || syntax::has_annotation(func, ctx.source(), "Override")
        {
            return;
        }
        let Some(class) = ctx.enclosing_class(func) else {
            return;
        };
        if kind(class) == "object_creation_expression" || kind(class) == "enum_constant" {
            return;
        }
        if ctx
            .ancestors(class)
            .any(|a| matches!(kind(a), "block" | "lambda_expression"))
        {
            return;
        }
        // Whether the method is used as a method reference is only known once the whole file
        // has been seen.
        self.parked.push((name, func));
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        let src = ctx.source();
        for &(name, method) in &self.parked {
            // Bound to a functional interface (declared elsewhere, like the lambdas above)
            // whose method may not declare the checked exception.
            if self.referenced.contains(&syntax::name_of(method, src))
                || helper_of_referenced(ctx, method, &self.referenced)
            {
                continue;
            }
            ctx.emit(
                name,
                "Let the exception propagate (or use assertDoesNotThrow) instead of catching it only to call fail()",
            );
        }
    }
}
