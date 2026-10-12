//! `java/nested-test-class-not-run` — a non-static inner class holding JUnit 5 tests but no
//! `@Nested`: JUnit 5 never runs those tests.

use super::util::{find_annotation, ImportGate};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["tests", "junit"],
    remediation_minutes: 1,
    description: r#"Detects a non-static inner class that declares JUnit 5 test methods (`@Test`,
`@ParameterizedTest`, `@RepeatedTest`, `@TestFactory`, `@TestTemplate`) but is not annotated
`@Nested`.

**Why it matters.** JUnit 5 only runs the tests of an inner (non-static member) class when the
class is marked `@Nested`. Without the annotation the class is skipped without a warning: the
build is green, the test report simply does not mention those tests, and the behaviour they
were written to check is not verified.

**How to fix.** Annotate the inner class with `@Nested`.

```java
// Noncompliant
class CartTest {
    class WhenEmpty {
        @Test
        void totalIsZero() { … }
    }
}

// Compliant
class CartTest {
    @Nested
    class WhenEmpty {
        @Test
        void totalIsZero() { … }
    }
}
```

Only inner classes are checked: whether a `static` nested class is run depends on the build
tool's test filters. Abstract inner classes (templates for `@Nested` subclasses) are not
reported, and only annotations that resolve to JUnit 5 through the imports count."#,
    ..RuleMeta::new(
        "java/nested-test-class-not-run",
        "java",
        "Inner test class without @Nested is never run",
    )
};

const TEST_FQNS: &[&str] = &[
    "org.junit.jupiter.api.Test",
    "org.junit.jupiter.params.ParameterizedTest",
    "org.junit.jupiter.api.RepeatedTest",
    "org.junit.jupiter.api.TestFactory",
    "org.junit.jupiter.api.TestTemplate",
];

#[derive(Default)]
pub struct NestedTestClassNotRun {
    jupiter: ImportGate,
}

impl<'t> JavaRule<'t> for NestedTestClassNotRun {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["class_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !self.jupiter.open(ctx, "org.junit.jupiter") {
            return;
        }
        // A member of a class (not of an interface, where members are implicitly static).
        let Some(body) = ctx.parent(node) else { return };
        if kind(body) != "class_body" {
            return;
        }
        let Some(outer) = ctx.parent(body) else {
            return;
        };
        if kind(outer) != "class_declaration" {
            return;
        }
        if syntax::has_modifier(node, "static") || syntax::has_modifier(node, "abstract") {
            return;
        }
        if syntax::annotations(node).any(|a| syntax::annotation_name(a, ctx.source()) == "Nested") {
            return;
        }
        let Some(own) = syntax::type_body(node) else {
            return;
        };
        let has_tests = ctx.members(own).any(|m| {
            kind(m) == "method_declaration" && find_annotation(ctx, m, TEST_FQNS).is_some()
        });
        if !has_tests {
            return;
        }
        let Some(name) = node.field(Field::Name) else {
            return;
        };
        ctx.emit(
            name,
            format!(
                "Annotate `{}` with @Nested; JUnit 5 skips the tests of an inner class without it",
                ctx.text(name)
            ),
        );
    }
}
