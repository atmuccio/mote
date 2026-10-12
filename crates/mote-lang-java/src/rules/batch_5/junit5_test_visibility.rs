//! `java/junit5-test-visibility` — `public` on JUnit 5 test classes and test/lifecycle methods.

use super::util::{self, Mods, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["tests", "junit"],
    remediation_minutes: 1,
    description: r#"Detects the `public` modifier on JUnit 5 test classes and on test and
lifecycle methods (`@Test`, `@ParameterizedTest`, `@RepeatedTest`, `@TestFactory`,
`@TestTemplate`, `@BeforeEach`, `@AfterEach`, `@BeforeAll`, `@AfterAll`).

**Why it matters.** JUnit 4 required public tests; JUnit 5 finds package-private classes and
methods just as well. The leftover `public` is noise, and it wrongly suggests that other code
is meant to call the test.

**How to fix.** Remove `public`.

```java
// Noncompliant
public class InvoiceTest {
    @BeforeEach
    public void setUp() { ... }

    @Test
    public void computesTotal() { ... }
}

// Compliant
class InvoiceTest {
    @BeforeEach
    void setUp() { ... }

    @Test
    void computesTotal() { ... }
}
```

Only test files that import `org.junit.jupiter` (and no JUnit 4 test API) are checked. A test
class is not reported when it is abstract or has other public members (constants, helpers,
nested types) that code elsewhere may use; `@Override` methods, interface methods, and
lifecycle methods of classes that extend another class (they may override a public hook) are
not reported."#,
    ..RuleMeta::new(
        "java/junit5-test-visibility",
        "java",
        "Public JUnit 5 test class or method",
    )
};

/// Test annotations (a class holding one of these is a test class).
const TEST_FQNS: &[&str] = &[
    "org.junit.jupiter.api.Test",
    "org.junit.jupiter.params.ParameterizedTest",
    "org.junit.jupiter.api.RepeatedTest",
    "org.junit.jupiter.api.TestFactory",
    "org.junit.jupiter.api.TestTemplate",
];

/// Lifecycle annotations.
const LIFECYCLE_FQNS: &[&str] = &[
    "org.junit.jupiter.api.BeforeEach",
    "org.junit.jupiter.api.AfterEach",
    "org.junit.jupiter.api.BeforeAll",
    "org.junit.jupiter.api.AfterAll",
];

const SIMPLE_NAMES: &[&str] = &[
    "Test",
    "ParameterizedTest",
    "RepeatedTest",
    "TestFactory",
    "TestTemplate",
    "BeforeEach",
    "AfterEach",
    "BeforeAll",
    "AfterAll",
];

/// JUnit 4 test API: files using it (mixed or vintage suites) need public tests.
fn uses_junit4_tests(ctx: &Ctx<'_>) -> bool {
    ctx.imports().iter().any(|i| {
        let p = i.path;
        !i.is_static
            && (p == "org.junit"
                || p == "junit.framework"
                || p.starts_with("junit.framework.")
                || p.starts_with("org.junit.runner")
                || p.starts_with("org.junit.rules")
                || matches!(
                    p,
                    "org.junit.Test"
                        | "org.junit.Before"
                        | "org.junit.After"
                        | "org.junit.BeforeClass"
                        | "org.junit.AfterClass"
                        | "org.junit.Rule"
                        | "org.junit.ClassRule"
                        | "org.junit.Ignore"
                ))
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Test,
    Lifecycle,
}

fn role(ctx: &Ctx<'_>, method: Node<'_>) -> Option<Role> {
    let src = ctx.source();
    for a in Mods::of(method).annotations() {
        if !SIMPLE_NAMES.contains(&syntax::annotation_name(a, src)) {
            continue;
        }
        if util::annotation_is(ctx, a, TEST_FQNS) {
            return Some(Role::Test);
        }
        if util::annotation_is(ctx, a, LIFECYCLE_FQNS) {
            return Some(Role::Lifecycle);
        }
    }
    None
}

#[derive(Default)]
pub struct Junit5TestVisibility {
    /// `Some(applies)` once the imports have been looked at.
    applies: Option<bool>,
}

impl Junit5TestVisibility {
    fn applies(&mut self, ctx: &Ctx<'_>) -> bool {
        *self.applies.get_or_insert_with(|| {
            ctx.is_test()
                && ctx
                    .imports()
                    .iter()
                    .any(|i| i.path.starts_with("org.junit.jupiter"))
                && !uses_junit4_tests(ctx)
        })
    }
}

impl<'t> JavaRule<'t> for Junit5TestVisibility {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_declaration", "class_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !ctx.is_test() {
            return;
        }
        let mods = Mods::of(node);
        let Some(public) = mods.keyword("public") else {
            return;
        };
        if !mods.annotated() && kind(node) == "method_declaration" {
            return;
        }
        if !self.applies(ctx) {
            return;
        }
        if kind(node) == "method_declaration" {
            if mods.has_annotation(ctx.source(), "Override") {
                return;
            }
            let Some(role) = role(ctx, node) else {
                return;
            };
            // Interface methods are public anyway.
            let Some(class) = ctx
                .enclosing_class(node)
                .filter(|c| kind(*c) == "class_declaration")
            else {
                return;
            };
            // A lifecycle method of a subclass may override (or hide) a public hook of the
            // base class, which must stay public.
            if role == Role::Lifecycle && util::fld(class, F.superclass).is_some() {
                return;
            }
            let what = match role {
                Role::Test => "test method",
                Role::Lifecycle => "lifecycle method",
            };
            ctx.emit(
                public,
                format!("Remove `public` from this JUnit 5 {what}; package-private is enough"),
            );
            return;
        }
        // A test class: not abstract, holds tests, and exposes nothing else publicly.
        if mods.has("abstract") {
            return;
        }
        let mut has_test = false;
        for m in util::class_members(ctx, node) {
            match kind(m) {
                "method_declaration" => match role(ctx, m) {
                    Some(Role::Test) => has_test = true,
                    Some(Role::Lifecycle) => {}
                    None => {
                        if Mods::of(m).has("public") {
                            return;
                        }
                    }
                },
                "constructor_declaration" => {}
                "class_declaration" if Mods::of(m).has_annotation(ctx.source(), "Nested") => {}
                _ => {
                    let mm = Mods::of(m);
                    if mm.has("public") || mm.has("protected") {
                        return;
                    }
                }
            }
        }
        if !has_test {
            return;
        }
        let name = ctx.name_of(node);
        ctx.emit(
            public,
            format!("Remove `public` from test class `{name}`; JUnit 5 does not need it"),
        );
    }
}
