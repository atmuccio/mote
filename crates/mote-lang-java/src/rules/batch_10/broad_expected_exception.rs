//! `java/broad-expected-exception` — `@Test(expected = X.class)` or an `ExpectedException` rule
//! covering several calls.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["tests", "junit"],
    remediation_minutes: 5,
    description: r#"Detects exception expectations that cover more than the one call meant to
throw: a JUnit 4 `@Test(expected = X.class)` (or TestNG `expectedExceptions`) on a test whose
body makes calls in more than one statement, and an `ExpectedException` rule (`thrown.expect(..)`)
followed by more than one statement that makes calls.

**Why it matters.** The expectation is met as soon as *any* of the covered code throws the
exception. When the setup or a second call can throw it too, the test passes for the wrong
reason and keeps passing when the code under test stops throwing. Statements after the call
that throws never run, so checks placed there (`verify(..)`, assertions) are silently skipped.

**How to fix.** Keep the setup outside the expectation: use `assertThrows` around the single
call expected to throw (JUnit 4.13+, JUnit 5), and put further checks after it.

```java
// Noncompliant
@Test(expected = IllegalStateException.class)
public void rejectsSecondStart() {
    Engine engine = new Engine(config);
    engine.start();
    engine.start();
}

// Compliant
@Test
public void rejectsSecondStart() {
    Engine engine = new Engine(config);
    engine.start();
    assertThrows(IllegalStateException.class, engine::start);
}
```

Only test files are checked. A statement counts when it calls a method or creates an object;
declarations with literal values, constructions without arguments (`new Parser()`), a final `fail(..)` (it throws an `AssertionError`, not the
expected exception), and calls inside lambdas or anonymous classes do not count."#,
    ..RuleMeta::new(
        "java/broad-expected-exception",
        "java",
        "Exception expectation covers several calls",
    )
};

pub struct BroadExpectedException;

/// `ExpectedException` methods that set up the expectation.
const EXPECT_METHODS: &[&str] = &["expect", "expectMessage", "expectCause"];

/// Whether evaluating `n` calls a method or creates an object (lambdas and anonymous class
/// bodies are not run by the statement itself).
fn makes_calls(n: Node<'_>) -> bool {
    let mut stack = vec![n];
    let mut steps = 0;
    while let Some(x) = stack.pop() {
        steps += 1;
        if steps > 4_000 {
            return true;
        }
        match kind(x) {
            // `new Parser()`: a trivial construction, not setup that could throw.
            "object_creation_expression"
                if syntax::argument_count(x) == 0
                    && syntax::child_of_kind(x, "class_body").is_none() => {}
            "method_invocation"
            | "object_creation_expression"
            | "explicit_constructor_invocation" => return true,
            "lambda_expression" | "class_body" | "method_reference" => continue,
            _ => stack.extend(named_children(x)),
        }
    }
    false
}

/// `fail(..)` / `Assert.fail(..)` as a statement.
fn is_fail<'t>(ctx: &Ctx<'t>, stmt: Node<'t>) -> bool {
    kind(stmt) == "expression_statement"
        && syntax::first_named_child(stmt)
            .is_some_and(|c| kind(c) == "method_invocation" && ctx.call_name(c) == Some("fail"))
}

/// The statements of `stmts` that make calls, not counting a final `fail(..)`.
fn calling_statements<'t>(ctx: &Ctx<'t>, stmts: &[Node<'t>]) -> usize {
    let mut n = stmts.iter().filter(|s| makes_calls(**s)).count();
    if stmts.last().is_some_and(|s| is_fail(ctx, *s)) {
        n = n.saturating_sub(1);
    }
    n
}

impl BroadExpectedException {
    /// `@Test(expected = X.class)` / `@Test(expectedExceptions = X.class)` on a test making calls
    /// in several statements.
    fn annotation<'t>(ctx: &Ctx<'t>, method: Node<'t>) {
        let Some(pair) = syntax::annotations(method)
            .filter(|a| syntax::annotation_name(*a, ctx.source()) == "Test")
            .filter_map(|a| a.field(Field::Arguments))
            .flat_map(named_children)
            .find(|p| {
                kind(*p) == "element_value_pair"
                    && p.field(Field::Key)
                        .is_some_and(|k| matches!(ctx.text(k), "expected" | "expectedExceptions"))
            })
        else {
            return;
        };
        let Some(body) = method.field(Field::Body) else {
            return;
        };
        let stmts: Vec<Node<'t>> = named_children(body).collect();
        let n = calling_statements(ctx, &stmts);
        if n >= 2 {
            ctx.emit(
                pair,
                format!(
                    "Expect the exception around the one call that should throw: this test makes \
                     calls in {n} statements, and any of them satisfies the expectation"
                ),
            )
            .value(n as f64);
        }
    }

    /// `thrown.expect(X.class);` followed by several statements that make calls.
    fn rule_call<'t>(ctx: &Ctx<'t>, call: Node<'t>) {
        let Some(receiver) = call.field(Field::Object) else {
            return;
        };
        if kind(receiver) != "identifier" && kind(receiver) != "field_access" {
            return;
        }
        let ident = if kind(receiver) == "field_access" {
            match (receiver.field(Field::Object), receiver.field(Field::Field)) {
                (Some(o), Some(f)) if kind(o) == "this" => f,
                _ => return,
            }
        } else {
            receiver
        };
        let Some(d) = ctx.declaration_of(ident) else {
            return;
        };
        let is_rule = d.type_node.is_some_and(|t| {
            let text = ctx.text(t);
            (text == "ExpectedException" || text == "org.junit.rules.ExpectedException")
                && ctx.resolve_type_name("ExpectedException", t).as_deref()
                    == Some("org.junit.rules.ExpectedException")
        });
        if !is_rule {
            return;
        }
        // The statement holding the call, directly in a block.
        let Some(stmt) = ctx
            .parent(call)
            .filter(|s| kind(*s) == "expression_statement")
        else {
            return;
        };
        let Some(block) = ctx.parent(stmt).filter(|b| kind(*b) == "block") else {
            return;
        };
        let after: Vec<Node<'t>> = named_children(block)
            .skip_while(|s| s.id() != stmt.id())
            .skip(1)
            .collect();
        // Only the last expectation of a run of them reports.
        let expects_again = after.iter().any(|s| {
            syntax::first_named_child(*s).is_some_and(|c| {
                kind(c) == "method_invocation"
                    && ctx
                        .call_name(c)
                        .is_some_and(|m| EXPECT_METHODS.contains(&m))
                    && c.field(Field::Object)
                        .is_some_and(|o| ctx.text(o) == ctx.text(receiver))
            })
        });
        if expects_again {
            return;
        }
        let n = calling_statements(ctx, &after);
        if n >= 2 {
            let name = ctx.call_name(call).unwrap_or("expect");
            ctx.emit(
                call,
                format!(
                    "Narrow this `{name}(..)`: {n} statements with calls follow it, and any of them \
                     satisfies the expectation (checks after the throwing call never run)"
                ),
            )
            .value(n as f64);
        }
    }
}

impl<'t> JavaRule<'t> for BroadExpectedException {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_declaration", "method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        EXPECT_METHODS
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !ctx.is_test() {
            return;
        }
        if kind(node) == "method_declaration" {
            Self::annotation(ctx, node);
        } else if ctx
            .call_name(node)
            .is_some_and(|m| EXPECT_METHODS.contains(&m))
        {
            Self::rule_call(ctx, node);
        }
    }
}
