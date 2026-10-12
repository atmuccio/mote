//! `java/test-assertion-in-production` — production code calls a test assertion library.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["tests", "dependencies"],
    remediation_minutes: 10,
    description: r#"Detects production code (not a test file) that calls the assertions of a test
library: JUnit's `Assert`/`Assertions`, AssertJ, Hamcrest's `assertThat`, TestNG, Truth.

**Why it matters.** Test libraries are usually test-scoped dependencies: in the packaged
application the class may be missing and the call fails with `NoClassDefFoundError`. Even
when present, their failures are `AssertionError`s, which ordinary `catch (Exception e)`
handlers do not catch, so an input check in production code can bring down a request thread
or a batch job instead of producing a proper error.

**How to fix.** Use a production check: `Objects.requireNonNull`, an explicit `if` that throws
`IllegalArgumentException` / `IllegalStateException`, or the validation utilities of the
framework in use.

```java
// Noncompliant
import static org.junit.Assert.assertNotNull;
void register(User user) {
    assertNotNull(user);
}

// Compliant
void register(User user) {
    Objects.requireNonNull(user, "user");
}
```

Only files of a `main` source set (`…/src/main/…`) are checked: other source sets
(`src/it`, `src/functionalTest`, …) hold tests. Test-support code is not reported either:
files whose path or package mentions `test`, `mock`, `fixture`, `example` or `sample`, files
under a `resources` folder (code templates), and classes whose name contains `Test`,
`Assert` or `Matcher`. Only the public assertion classes of the libraries count (JUnit's
`Assert`/`Assertions`/`TestCase`, AssertJ's `Assertions` family and `Fail`, Hamcrest's
`MatcherAssert`, TestNG's `Assert`/`AssertJUnit`, Truth): a library's internal helpers named
`assert…` are not test assertions, and the libraries' own sources (files in their packages)
are not checked."#,
    ..RuleMeta::new(
        "java/test-assertion-in-production",
        "java",
        "Test assertion in production code",
    )
};

#[derive(Default)]
pub struct TestAssertionInProduction {
    /// The file is production code that imports a test assertion library.
    applies: Option<bool>,
}

fn applies(ctx: &Ctx<'_>) -> bool {
    if ctx.is_test() {
        return false;
    }
    let lower = ctx.path().to_ascii_lowercase();
    // Production code lives in a `main` source set; other sets (`aat`, `it`, `functionalTest`)
    // hold tests of their own.
    if !(lower.starts_with("src/main/") || lower.contains("/src/main/")) {
        return false;
    }
    if [
        "test",
        "mock",
        "fixture",
        "/resources/",
        "example",
        "sample",
    ]
    .iter()
    .any(|w| lower.contains(w))
    {
        return false;
    }
    if ctx.package_name().is_some_and(|p| {
        p.to_ascii_lowercase().contains("test")
            // The assertion library's own sources (`org.assertj.core.internal`).
            || util::ASSERTION_PACKAGES
                .iter()
                .any(|lib| p.starts_with(lib) || p == lib.trim_end_matches('.'))
    }) {
        return false;
    }
    ctx.imports().iter().any(|i| {
        util::ASSERTION_PACKAGES
            .iter()
            .any(|p| i.path.starts_with(p))
    })
}

impl<'t> JavaRule<'t> for TestAssertionInProduction {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &[
            "assertEquals",
            "assertNotEquals",
            "assertTrue",
            "assertFalse",
            "assertNull",
            "assertNotNull",
            "assertSame",
            "assertNotSame",
            "assertThat",
            "assertArrayEquals",
            "assertThrows",
            "assertAll",
            "fail",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) = ctx.call_name(node) else {
            return;
        };
        if !*self.applies.get_or_insert_with(|| applies(ctx)) {
            return;
        }
        if !util::is_test_library_call(ctx, node, name) {
            return;
        }
        if ctx.enclosing_class(node).is_some_and(|c| {
            let n = syntax::name_of(c, ctx.source());
            n.contains("Test") || n.contains("Assert") || n.contains("Matcher")
        }) {
            return;
        }
        ctx.emit(
            node.field(Field::Name).unwrap_or(node),
            format!("Replace the test assertion `{name}` with a production check (it throws AssertionError and needs a test library at run time)"),
        );
    }
}
