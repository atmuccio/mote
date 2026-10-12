//! `java/too-many-assertions` — a test method with more assertions than a threshold.

use super::common::{self, Walk};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["tests", "size"],
    params: &[ParamSpec {
        name: "max",
        default: ParamDefault::Int(25),
        description: "Most assertions allowed in one test method.",
    }],
    remediation_minutes: 20,
    description: r#"Detects test methods (`@Test`, `@ParameterizedTest`, `@RepeatedTest`, and JUnit 3
`test…` methods) that contain more assertions than `max` (25 by default). An assertion is a
call whose name starts with `assert` (`assertEquals`, `assertThat`, `assertThrows`, soft
assertions…; a fluent chain counts once) or a Java `assert` statement.

**Why it matters.** A test with dozens of assertions checks many behaviors at once. When it
fails, it stops at the first broken expectation and hides the state of all the others; its
name cannot say what it verifies; and every change to one of the behaviors touches the same
long method. Such tests usually grew by appending cases.

**How to fix.** Split the method into tests that each check one behavior, or turn repeated
checks into a parameterized test or a table of cases.

```java
// Noncompliant
@Test
void parser() {
    assertEquals(1, parse("1"));
    assertEquals(2, parse("2"));
    // … 40 more
}

// Compliant
@ParameterizedTest
@CsvSource({"1, 1", "2, 2"})
void parsesNumbers(String text, int expected) {
    assertEquals(expected, parse(text));
}
```

The count is static: an assertion inside a loop counts once. Only test code and files that
import JUnit's or TestNG's `@Test` are looked at. Assertion calls inside helper methods are not
counted, and neither are verifications of mocks (`verify`)."#,
    ..RuleMeta::new(
        "java/too-many-assertions",
        "java",
        "Test method with too many assertions",
    )
};

#[derive(Default)]
pub struct TooManyAssertions {
    /// The `max` parameter, read once per file.
    max: Option<usize>,
    /// Whether the file can hold tests: test code, or an import of a test framework's
    /// annotations (read once per file).
    tests: Option<bool>,
}

/// Packages whose `@Test` annotations mark test methods.
const TEST_PACKAGES: &[&str] = &[
    "org.junit",
    "org.junit.jupiter.api",
    "org.testng.annotations",
];

/// Whether a call counts as an assertion: `assert…` names, counted once per fluent chain
/// (`assertThat(x).isEqualTo(y)` is one; its receiver chain holds the `assert` call).
fn is_assertion<'t>(ctx: &Ctx<'t>, call: Node<'t>) -> bool {
    ctx.call_name(call)
        .is_some_and(|n| n.starts_with("assert") && n != "assertAll")
}

impl<'t> JavaRule<'t> for TooManyAssertions {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        // Cheapest tests first: a file that can hold tests, a test annotation in the modifiers
        // (or a JUnit 3 `test…` name in test code), the body's size, then the occurrences of
        // the word `assert`.
        let tests = *self.tests.get_or_insert_with(|| {
            ctx.is_test()
                || ctx.imports().iter().any(|i| {
                    !i.is_static
                        && TEST_PACKAGES.iter().any(|p| {
                            i.path == *p
                                || i.path.strip_prefix(*p).is_some_and(|r| r.starts_with('.'))
                        })
                })
        });
        if !tests {
            return;
        }
        let src = ctx.source();
        let annotated = syntax::modifiers(node).is_some_and(|mods| {
            ctx.text(mods).contains("Test")
                && named_children(mods).any(|a| {
                    matches!(kind(a), "annotation" | "marker_annotation")
                        && matches!(
                            syntax::annotation_name(a, src),
                            "Test" | "ParameterizedTest" | "RepeatedTest"
                        )
                })
        });
        let is_test = annotated
            || (ctx.is_test()
                && node
                    .field(Field::Name)
                    .is_some_and(|n| ctx.text(n).starts_with("test")));
        if !is_test {
            return;
        }
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        // An assertion takes at least eight characters (`assert b;`): a shorter body cannot
        // exceed `max`.
        let max = *self
            .max
            .get_or_insert_with(|| ctx.param_i64("max").max(0) as usize);
        if body.end_byte() - body.start_byte() < max.saturating_mul(8) {
            return;
        }
        if ctx.text(body).matches("assert").count() <= max {
            return;
        }
        let mut count = 0usize;
        common::walk(body, |n| {
            match kind(n) {
                "assert_statement" => count += 1,
                "method_invocation" if is_assertion(ctx, n) => count += 1,
                // Local and anonymous class bodies are other methods.
                "class_body" => return Walk::Skip,
                _ => {}
            }
            Walk::Continue
        });
        if count <= max {
            return;
        }
        let Some(name) = ctx.name_node(node) else {
            return;
        };
        ctx.emit(
            name,
            format!(
                "Split this test: it makes {count} assertions (more than {max}) and stops at the first failure"
            ),
        )
        .value(count as f64);
    }
}
