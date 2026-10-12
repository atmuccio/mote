//! `java/mocks-not-initialized` — a test class with `@Mock` fields and nothing that ever
//! creates the mocks.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["tests", "mockito"],
    remediation_minutes: 2,
    description: r#"Detects test classes whose fields carry Mockito's `@Mock`, `@Spy`, `@Captor` or
`@InjectMocks` while nothing in the file asks Mockito to process them: no
`@ExtendWith(MockitoExtension.class)`, no `@RunWith(MockitoJUnitRunner.class)`, no
`MockitoJUnit.rule()`, no `MockitoAnnotations.openMocks(this)` / `initMocks(this)`.

**Why it matters.** The annotations do nothing by themselves: some runner, extension, rule or
explicit call must create the mocks. Without it the fields stay `null`; tests that use them
fail with a `NullPointerException` far from the cause, and tests that only pass them along
(to `@InjectMocks` or a constructor) silently test against `null` collaborators.

**How to fix.** Add the Mockito extension (JUnit 5) or runner (JUnit 4), or initialize the
mocks in a set-up method.

```java
// Noncompliant
class OrderServiceTest {
    @Mock OrderRepository repository;
    @Test void saves() { … }
}

// Compliant
@ExtendWith(MockitoExtension.class)
class OrderServiceTest {
    @Mock OrderRepository repository;
    @Test void saves() { … }
}
```

Only top-level, non-abstract classes that declare `@Test` methods and extend nothing are
checked (a superclass may set the mocks up); classes with any annotation other than a few
descriptive ones (`@DisplayName`, `@Tag`, `@Disabled`, …) are skipped, since a custom
annotation may carry the extension, and so are files that mention Spring's test support."#,
    ..RuleMeta::new(
        "java/mocks-not-initialized",
        "java",
        "Mockito annotations never processed",
    )
};

#[derive(Default)]
pub struct MocksNotInitialized {
    /// Whether the file uses mock annotations, initializes no mocks and imports Mockito: the
    /// same for every class of the file, so tested once (scanning the whole source per class
    /// was quadratic in a file of many classes).
    file_qualifies: Option<bool>,
}

const MOCK_ANNOTATIONS: &[&str] = &["Mock", "Spy", "Captor", "InjectMocks"];

/// Class annotations that cannot bring in a Mockito extension.
const HARMLESS: &[&str] = &[
    "DisplayName",
    "DisplayNameGeneration",
    "Tag",
    "Tags",
    "TestInstance",
    "Disabled",
    "Ignore",
    "SuppressWarnings",
    "Timeout",
    "TestMethodOrder",
    "FixMethodOrder",
    "Execution",
    "Order",
    "Deprecated",
    "Category",
];

/// Anything in the file that may initialize the annotated fields.
const INITIALIZERS: &[&str] = &[
    "MockitoAnnotations",
    "MockitoExtension",
    "MockitoJUnitRunner",
    "MockitoJUnit",
    "MockitoTestNGListener",
    "MockitoSession",
    "MockitoSettings",
    "openMocks",
    "initMocks",
    "Spring",
    "Quarkus",
    "Micronaut",
];

impl<'t> JavaRule<'t> for MocksNotInitialized {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["class_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if ctx.parent(node).is_none_or(|p| kind(p) != "program") {
            return;
        }
        let src = ctx.source();
        let file_qualifies = *self.file_qualifies.get_or_insert_with(|| {
            let mocks = src.contains("@Mock")
                || src.contains("@Spy")
                || src.contains("@Captor")
                || src.contains("@InjectMocks");
            let mockito_imported = ctx.imports().iter().any(|i| {
                !i.is_static
                    && (i.path == "org.mockito"
                        || MOCK_ANNOTATIONS
                            .iter()
                            .any(|a| i.path.strip_prefix("org.mockito.") == Some(a)))
            });
            mocks && !INITIALIZERS.iter().any(|w| src.contains(w)) && mockito_imported
        });
        if !file_qualifies
            || node.field(Field::Superclass).is_some()
            || syntax::has_modifier(node, "abstract")
        {
            return;
        }
        if syntax::annotations(node).any(|a| !HARMLESS.contains(&syntax::annotation_name(a, src))) {
            return;
        }
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        let has_test = ctx.members(body).any(|m| {
            kind(m) == "method_declaration"
                && syntax::annotations(m).any(|a| {
                    matches!(
                        syntax::annotation_name(a, src),
                        "Test" | "ParameterizedTest" | "RepeatedTest" | "TestFactory"
                    )
                })
        });
        if !has_test {
            return;
        }
        let Some(first) = ctx
            .members(body)
            .filter(|m| kind(*m) == "field_declaration")
            .flat_map(syntax::annotations)
            .find(|a| {
                let n = ctx.text(a.field(Field::Name).unwrap_or(*a));
                MOCK_ANNOTATIONS.contains(&n)
                    || n.strip_prefix("org.mockito.")
                        .is_some_and(|s| MOCK_ANNOTATIONS.contains(&s))
            })
        else {
            return;
        };
        let name = syntax::annotation_name(first, src);
        ctx.emit(
            first,
            format!("Nothing initializes the `@{name}` fields of this class: add `@ExtendWith(MockitoExtension.class)` or call `MockitoAnnotations.openMocks(this)`"),
        );
    }
}
