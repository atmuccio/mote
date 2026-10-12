//! `java/test-class-naming` — a test class whose name neither starts with `Test` nor ends with
//! `Test`, `Tests`, `TestCase` or `IT` (the class names Maven runs by default).

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["conventions", "naming", "tests"],
    params: &[
        ParamSpec {
            name: "suffixes",
            default: ParamDefault::Str("Test,Tests,TestCase,IT"),
            description: "Comma-separated name endings accepted for test classes.",
        },
        ParamSpec {
            name: "prefixes",
            default: ParamDefault::Str("Test"),
            description: "Comma-separated name beginnings also accepted (empty: endings only).",
        },
    ],
    remediation_minutes: 2,
    description: r#"Detects top-level test classes, classes with JUnit or TestNG test methods,
whose name neither starts with `Test` nor ends with `Test`, `Tests`, `TestCase` or `IT`.

**Why it matters.** A test class named after the class it tests plus `Test` (`InvoiceParser`
→ `InvoiceParserTest`) is found at once from either side and sorts next to its subject. The
name also matters to the build: by default, Maven's test plugins only run the classes whose
names match their include patterns — `Test*`, `*Test`, `*Tests` and `*TestCase` for unit
tests, `*IT` for integration tests — so a test class named `InvoiceParserChecks` may silently
never run.

**How to fix.** Rename the class (and its file) to match, ideally the name of the class under
test followed by `Test`.

```java
// Noncompliant
class InvoiceParsing {
    @Test
    void readsTotals() { … }
}

// Compliant
class InvoiceParserTest {
    @Test
    void readsTotals() { … }
}
```

Recognized test methods carry the `@Test`, `@ParameterizedTest`, `@RepeatedTest`,
`@TestFactory` or `@TestTemplate` annotation of JUnit 4, JUnit 5 or TestNG (resolved through
the imports); a TestNG `@Test` on the class also counts. Not reported: shared base classes,
whose tests their subclasses run under their own names (abstract classes, and classes named
`…TestBase`, `…TestSupport` or `Abstract…` — `Abstract` as a whole first word — even when they
are not declared abstract); nested classes (JUnit 5 `@Nested` groups); and whether the name
matches the class under test, which needs the rest of the project. The parameter `suffixes`
sets the accepted endings and `prefixes` the accepted beginnings; set `prefixes` to an empty
string to accept endings only, for a team that does not use the `TestInvoiceParser` style.
The rule is off by default: it is part of the `conventions` profile."#,
    ..RuleMeta::new(
        "java/test-class-naming",
        "java",
        "Test class name without a test prefix or suffix",
    )
};

const TEST_ANNOTATIONS: &[&str] = &[
    "org.junit.Test",
    "org.junit.jupiter.api.Test",
    "org.junit.jupiter.params.ParameterizedTest",
    "org.junit.jupiter.api.RepeatedTest",
    "org.junit.jupiter.api.TestFactory",
    "org.junit.jupiter.api.TestTemplate",
    "org.testng.annotations.Test",
];

fn is_test_annotation(ctx: &Ctx<'_>, ann: Node<'_>) -> bool {
    matches!(
        syntax::annotation_name(ann, ctx.source()),
        "Test" | "ParameterizedTest" | "RepeatedTest" | "TestFactory" | "TestTemplate"
    ) && util::annotation_is(ctx, ann, TEST_ANNOTATIONS)
}

/// Per file: whether the file can use a test framework at all (an import, or a qualified name).
#[derive(Default)]
pub struct TestClassNaming {
    framework: Option<bool>,
}

/// A name that marks a shared base class of tests, which subclasses run under their own
/// names: `…TestBase`, `…TestSupport`, or `Abstract` as the first word (`AbstractParserChecks`,
/// not `AbstractionChecks`).
fn is_base_class_name(name: &str) -> bool {
    name.ends_with("TestBase")
        || name.ends_with("TestSupport")
        || name
            .strip_prefix("Abstract")
            .and_then(|rest| rest.chars().next())
            .is_some_and(|c| c.is_uppercase() || c.is_ascii_digit() || c == '_')
}

fn uses_test_framework(ctx: &Ctx<'_>) -> bool {
    ctx.imports()
        .iter()
        .any(|i| i.path.starts_with("org.junit") || i.path.starts_with("org.testng"))
        || ctx.source().contains("@org.junit.")
        || ctx.source().contains("@org.testng.")
}

impl<'t> JavaRule<'t> for TestClassNaming {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["class_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        // Cheapest first: top-level classes of files that can use a test framework.
        if ctx.parent(node).is_none_or(|p| kind(p) != "program") {
            return;
        }
        if !*self
            .framework
            .get_or_insert_with(|| uses_test_framework(ctx))
        {
            return;
        }
        let Some(name) = ctx.name_node(node) else {
            return;
        };
        let class_name = ctx.text(name);
        let mut suffixes = ctx
            .param_str("suffixes")
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty());
        if suffixes.clone().any(|s| class_name.ends_with(s))
            || ctx
                .param_str("prefixes")
                .split(',')
                .map(str::trim)
                .any(|p| !p.is_empty() && class_name.starts_with(p))
        {
            return;
        }
        let suffix = suffixes.next().unwrap_or("Test");
        if is_base_class_name(class_name) || syntax::has_modifier(node, "abstract") {
            return;
        }
        let is_test_class = syntax::annotations(node).any(|a| is_test_annotation(ctx, a))
            || node.field(Field::Body).is_some_and(|body| {
                ctx.members(body).any(|m| {
                    kind(m) == "method_declaration"
                        && syntax::annotations(m).any(|a| is_test_annotation(ctx, a))
                })
            });
        if !is_test_class {
            return;
        }
        ctx.emit(
            name,
            format!("Rename the test class `{class_name}` so that it ends with `{suffix}`"),
        );
    }
}
