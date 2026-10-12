//! `java/test-lifecycle-method-not-run` — JUnit set-up / tear-down methods declared so that
//! JUnit refuses or ignores them.

use super::util::{annotation_is, ImportGate};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["tests", "junit"],
    remediation_minutes: 2,
    description: r#"Detects JUnit set-up and tear-down methods that JUnit will not run as intended:

- a per-test hook (`@BeforeEach`/`@AfterEach`, JUnit 4 `@Before`/`@After`) declared `static`;
- a per-class hook (JUnit 4 `@BeforeClass`/`@AfterClass`; JUnit 5 `@BeforeAll`/`@AfterAll`
  outside a `@TestInstance` class) that is not `static`;
- a hook that is `private` or returns a value, or a JUnit 4 hook that is not `public`;
- a hook of one JUnit generation in a class whose tests all belong to the other (a JUnit 4
  `@Before` next to JUnit 5 `@Test` methods): the engine running the tests does not know the
  hook.

**Why it matters.** Depending on the JUnit version, such a hook either makes the whole test
class fail with an initialization error or is silently skipped, so the tests run without the
set-up they rely on (or leave behind what the tear-down should have cleaned). Mixed JUnit 4 and
JUnit 5 annotations are the classic leftover of a half-finished migration: the hook compiles,
looks right, and never runs.

**How to fix.** Declare per-class hooks `static void` and per-test hooks non-static `void`,
not `private` (`public` for JUnit 4), and use the lifecycle annotations of the JUnit
generation the class's tests use.

```java
// Noncompliant
import org.junit.Before;
import org.junit.jupiter.api.Test;

class ParserTest {
    @Before                       // JUnit 4 hook, JUnit 5 tests: never runs
    void setUp() { parser = new Parser(); }

    @Test
    void parsesEmptyInput() { … }
}

// Compliant
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

class ParserTest {
    @BeforeEach
    void setUp() { parser = new Parser(); }

    @Test
    void parsesEmptyInput() { … }
}
```

Only annotations that resolve to JUnit through the imports count. A non-static JUnit 5
`@BeforeAll`/`@AfterAll` is not reported when the class or an enclosing class carries
`@TestInstance`, when the class extends another class (which may carry it), or in interfaces;
hooks marked `@Override` are left alone, and so are non-public `setUp()`/`tearDown()` hooks
of a class that extends another class (it may be a JUnit 3 `TestCase`, whose mechanism calls
the protected override itself). A project that switches every test class to the
per-class lifecycle in `junit-platform.properties` should turn the rule off."#,
    ..RuleMeta::new(
        "java/test-lifecycle-method-not-run",
        "java",
        "JUnit set-up or tear-down method that does not run",
    )
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Gen {
    Four,
    Five,
}

/// Lifecycle annotations: (fqn, generation, runs once per class).
const HOOKS: &[(&str, Gen, bool)] = &[
    ("org.junit.Before", Gen::Four, false),
    ("org.junit.After", Gen::Four, false),
    ("org.junit.BeforeClass", Gen::Four, true),
    ("org.junit.AfterClass", Gen::Four, true),
    ("org.junit.jupiter.api.BeforeEach", Gen::Five, false),
    ("org.junit.jupiter.api.AfterEach", Gen::Five, false),
    ("org.junit.jupiter.api.BeforeAll", Gen::Five, true),
    ("org.junit.jupiter.api.AfterAll", Gen::Five, true),
];

const TESTS_4: &[&str] = &["org.junit.Test"];
const TESTS_5: &[&str] = &[
    "org.junit.jupiter.api.Test",
    "org.junit.jupiter.params.ParameterizedTest",
    "org.junit.jupiter.api.RepeatedTest",
    "org.junit.jupiter.api.TestFactory",
    "org.junit.jupiter.api.TestTemplate",
];

#[derive(Default)]
pub struct TestLifecycleMethodNotRun<'t> {
    junit: ImportGate,
    /// Per class body (node id): whether it declares JUnit 4 / JUnit 5 tests.
    generations: FxHashMap<usize, (bool, bool)>,
    /// Whether an annotation name, as written, resolves to one of [`TESTS_4`] (`false`) or
    /// [`TESTS_5`] (`true`): the answer depends on the text and the imports only.
    test_names: FxHashMap<(&'t str, bool), bool>,
}

impl<'t> JavaRule<'t> for TestLifecycleMethodNotRun<'t> {
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
        let Some(name) = ctx.name_node(node) else {
            return;
        };
        // Cheap text test first: the header (annotations, modifiers, type) names a hook.
        let hooked = src
            .get(node.start_byte()..name.start_byte())
            .is_some_and(names_hook);
        if !hooked {
            return;
        }
        let mut hook = None;
        for ann in syntax::annotations(node) {
            let simple = syntax::annotation_name(ann, src);
            if simple == "Override" {
                return;
            }
            if hook.is_some() {
                continue;
            }
            for (fqn, generation, per_class) in HOOKS {
                if fqn.rsplit('.').next() == Some(simple) && annotation_is(ctx, ann, &[fqn]) {
                    hook = Some((ann, *generation, *per_class));
                    break;
                }
            }
        }
        let Some((ann, generation, per_class)) = hook else {
            return;
        };
        let Some(body) = ctx.parent(node) else { return };
        let Some(class) = ctx.parent(body) else {
            return;
        };
        if kind(class) != "class_declaration" {
            return;
        }
        let is_static = syntax::has_modifier(node, "static");
        let method = ctx.text(name);
        let problem = if syntax::has_modifier(node, "private") {
            Some("it is private".to_string())
        } else if generation == Gen::Four
            && !syntax::has_modifier(node, "public")
            && !junit3_hook_override(class, node, method)
        {
            Some("JUnit 4 hooks must be public".to_string())
        } else if node
            .field(Field::Type)
            .is_some_and(|t| kind(t) != "void_type")
        {
            Some("it returns a value".to_string())
        } else if !per_class && is_static {
            Some("a per-test hook must not be static".to_string())
        } else if per_class && !is_static && needs_static(class, generation, ctx) {
            Some("a per-class hook must be static".to_string())
        } else {
            self.other_generation_tests(class, body, generation, ctx)
                .then(|| {
                    let (hook_gen, test_gen) = match generation {
                        Gen::Four => ("JUnit 4", "JUnit 5"),
                        Gen::Five => ("JUnit 5", "JUnit 4"),
                    };
                    format!(
                        "it is a {hook_gen} hook and the tests of this class are {test_gen} tests"
                    )
                })
        };
        let Some(problem) = problem else { return };
        ctx.emit(
            ann,
            format!("Fix the declaration of `{method}`; JUnit does not run it as a lifecycle hook because {problem}"),
        );
    }
}

/// Whether a non-static per-class hook is an error here: always for JUnit 4; for JUnit 5
/// unless a `@TestInstance` may apply (on this or an enclosing class, or inherited).
fn needs_static<'t>(class: Node<'t>, generation: Gen, ctx: &Ctx<'t>) -> bool {
    if generation == Gen::Four {
        return true;
    }
    if class.field(Field::Superclass).is_some() {
        return false;
    }
    let src = ctx.source();
    let has_instance = |c: Node<'t>| {
        syntax::annotations(c).any(|a| syntax::annotation_name(a, src) == "TestInstance")
    };
    if has_instance(class) {
        return false;
    }
    !ctx.ancestors(class).any(|a| {
        syntax::is_type_declaration(a) && (kind(a) != "class_declaration" || has_instance(a))
    })
}

impl<'t> TestLifecycleMethodNotRun<'t> {
    /// Whether the class declares tests, all of the other JUnit generation (and inherits none
    /// that the hook could serve). The generations are computed once per class body.
    fn other_generation_tests(
        &mut self,
        class: Node<'t>,
        body: Node<'t>,
        generation: Gen,
        ctx: &Ctx<'t>,
    ) -> bool {
        if class.field(Field::Superclass).is_some() {
            return false;
        }
        let (four, five) = match self.generations.get(&body.id()) {
            Some(&(four, five)) => (four, five),
            None => {
                let (mut four, mut five) = (false, false);
                for m in ctx.members(body) {
                    if kind(m) != "method_declaration" {
                        continue;
                    }
                    if has_test(ctx, &mut self.test_names, m, false) {
                        four = true;
                    } else if has_test(ctx, &mut self.test_names, m, true) {
                        five = true;
                    }
                }
                self.generations.insert(body.id(), (four, five));
                (four, five)
            }
        };
        match generation {
            Gen::Four => five && !four,
            Gen::Five => four && !five,
        }
    }
}

/// Whether `m` carries a test annotation of [`TESTS_5`] (`jupiter`) or [`TESTS_4`]: the same
/// answer as `find_annotation(ctx, m, list).is_some()`, with the import resolution of each
/// annotation name cached in `names`.
fn has_test<'t>(
    ctx: &Ctx<'t>,
    names: &mut FxHashMap<(&'t str, bool), bool>,
    m: Node<'t>,
    jupiter: bool,
) -> bool {
    let fqns = if jupiter { TESTS_5 } else { TESTS_4 };
    syntax::annotations(m).any(|a| {
        let simple = syntax::annotation_name(a, ctx.source());
        let simple = simple.rsplit('.').next().unwrap_or(simple);
        if !fqns.iter().any(|f| f.rsplit('.').next() == Some(simple)) {
            return false;
        }
        let Some(name) = a.field(Field::Name) else {
            return false;
        };
        *names
            .entry((ctx.text(name), jupiter))
            .or_insert_with(|| annotation_is(ctx, a, fqns))
    })
}

/// Whether a method header holds an annotation whose simple name starts with `Before` or
/// `After` (`@BeforeEach`, `@org.junit.After`). A byte scan: this runs for every method.
fn names_hook(header: &str) -> bool {
    let bytes = header.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'@' {
            i += 1;
            continue;
        }
        let start = i + 1;
        let mut end = start;
        while end < bytes.len()
            && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_' || bytes[end] == b'.')
        {
            end += 1;
        }
        let segment_start = bytes[start..end]
            .iter()
            .rposition(|b| *b == b'.')
            .map_or(start, |p| start + p + 1);
        let simple = &bytes[segment_start..end];
        if simple.starts_with(b"Before") || simple.starts_with(b"After") {
            return true;
        }
        i = end.max(start);
    }
    false
}

/// Whether a non-public `setUp()` / `tearDown()` may override the hook of a JUnit 3 base
/// class: the class extends another class (which may be a `TestCase` declared in another
/// file), and the JUnit 3 mechanism calls the protected override itself, whatever the
/// annotation says.
fn junit3_hook_override(class: Node<'_>, method: Node<'_>, name: &str) -> bool {
    matches!(name, "setUp" | "tearDown")
        && class.field(Field::Superclass).is_some()
        && syntax::parameters(method).next().is_none()
}
