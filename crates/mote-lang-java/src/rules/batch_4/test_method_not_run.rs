//! `java/test-method-not-run` — JUnit `@Test` methods that are private, static, or non-void.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["tests", "junit"],
    remediation_minutes: 2,
    description: r#"Detects JUnit test methods (`@Test`, and JUnit 5 `@ParameterizedTest` /
`@RepeatedTest`) that are `private`, `static`, or return a value.

**Why it matters.** JUnit only executes non-private, non-static methods returning `void`.
JUnit 5 silently skips a test method that breaks these rules, so the test *looks* present
and green but never runs; JUnit 4 rejects the whole class at run time. Either way the
behaviour the test was written to protect is unverified.

**How to fix.** Make the method an instance method returning `void` that is not `private`
(JUnit 4 additionally needs `public`). Move any shared logic that returns a value into a
separate helper method.

```java
// Noncompliant
@Test
private void parsesDates() { … }

@Test
int computesTotal() { return assertTotal(); }

// Compliant
@Test
void parsesDates() { … }
```

Only annotations that resolve to JUnit 4 or JUnit 5 (through imports or a qualified name)
are considered; `@TestFactory` methods legitimately return values and are not checked."#,
    ..RuleMeta::new(
        "java/test-method-not-run",
        "java",
        "Test method that JUnit will not run",
    )
};

const TEST_ANNOTATIONS: &[&str] = &[
    "org.junit.Test",
    "org.junit.jupiter.api.Test",
    "org.junit.jupiter.params.ParameterizedTest",
    "org.junit.jupiter.api.RepeatedTest",
];

#[derive(Default)]
pub struct TestMethodNotRun {
    junit: util::Gate,
}

impl<'t> JavaRule<'t> for TestMethodNotRun {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !self.junit.open(ctx, "org.junit") {
            return;
        }
        let src = ctx.source();
        // Annotations precede the name: skip methods whose header has none.
        let Some(name) = ctx.name_node(node) else {
            return;
        };
        if !src
            .get(node.start_byte()..name.start_byte())
            .is_some_and(|h| h.contains('@'))
        {
            return;
        }
        let is_test = syntax::annotations(node).any(|a| {
            matches!(
                syntax::annotation_name(a, src),
                "Test" | "ParameterizedTest" | "RepeatedTest"
            ) && util::annotation_is(ctx, a, TEST_ANNOTATIONS)
        });
        if !is_test {
            return;
        }
        let reason = if syntax::has_modifier(node, "private") {
            "it is private"
        } else if syntax::has_modifier(node, "static") {
            "it is static"
        } else if util::fld(node, F::Type).is_some_and(|t| kind(t) != "void_type") {
            "it returns a value"
        } else {
            return;
        };
        let method = ctx.text(name);
        ctx.emit(
            name,
            format!("Make test `{method}` a non-private, non-static method returning void: JUnit does not run it because {reason}"),
        );
    }
}
