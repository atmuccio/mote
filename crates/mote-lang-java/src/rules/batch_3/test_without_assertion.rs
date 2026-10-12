//! `java/test-without-assertion` — a JUnit/TestNG test that asserts nothing.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Medium,
    tags: &["tests"],
    remediation_minutes: 10,
    description: r#"Detects `@Test` methods (JUnit 4, JUnit 5, TestNG) that contain no assertion
and no interaction verification.

**Why it matters.** A test without an assertion passes as long as nothing throws. It adds to
the coverage numbers while checking almost nothing, so regressions in the results go unnoticed
and the suite gives false confidence.

**How to fix.** Assert on the outcome (`assertEquals`, AssertJ `assertThat(…)`, Hamcrest
`assertThat(…, is(…))`), verify the expected interactions (Mockito `verify(…)`), or state the
expected exception (`assertThrows`, `@Test(expected = …)`). If the test really only checks that
no exception is thrown, say so with `assertDoesNotThrow(…)`.

```java
// Noncompliant
@Test
void parsesHeader() {
    parser.parse("id;name");
}

// Compliant
@Test
void parsesHeader() {
    assertEquals(List.of("id", "name"), parser.parse("id;name").columns());
}
```

A call counts as an assertion when its name starts with `assert`, `verify`, `expect`, `fail`,
`should`, or `then` (except Mockito stubbing such as `thenReturn`), or contains
`Assert`/`Verify`/`Expect` (`andExpect`, `softly.assertAll`); `assert` and `throw` statements
count too. Calls to helper methods declared in the same file are followed, and so are lambdas
stored in fields of the file and passed to a call (`run(CHECK_ENTRIES)` with
`static final Check CHECK_ENTRIES = f -> assertTrue(..)`); unqualified calls that may reach
an inherited or statically imported helper are assumed to assert, and so are calls on a
tester object — a chain starting at a class or variable whose type is named `…Tester`,
`…Verifier`, `…Assert`, `…Asserts` or `…Assertions` (`Type4Tester.create("1 2 add").pop(3)`),
whose methods check what they are given. Disabled tests
and tests with `expected`/`expectedExceptions` are not reported. Only test files are checked."#,
    ..RuleMeta::new(
        "java/test-without-assertion",
        "java",
        "Test without assertion",
    )
};

/// How deep same-file helper calls are followed.
const MAX_DEPTH: u32 = 3;

/// Per-file state (rule instances are fresh for every file).
#[derive(Default)]
pub struct TestWithoutAssertion<'t> {
    state: State<'t>,
    imports: StaticImports<'t>,
    /// Whether an annotation name, as written, is a test annotation (see `util::test_annotation`).
    test_names: FxHashMap<&'t str, bool>,
}

/// What the file's static imports may contribute (computed once per file).
#[derive(Default)]
struct StaticImports<'t> {
    ready: bool,
    /// An on-demand static import of a non-library class: any unqualified call may be a helper.
    open: bool,
    /// Simple names of single static imports from non-library classes.
    helpers: FxHashSet<&'t str>,
}

impl<'t> StaticImports<'t> {
    fn init(&mut self, ctx: &Ctx<'t>) {
        if self.ready {
            return;
        }
        self.ready = true;
        for i in ctx
            .imports()
            .iter()
            .filter(|i| i.is_static && !is_library(i.path))
        {
            if i.is_wildcard {
                self.open = true;
            } else {
                self.helpers.insert(i.simple_name());
            }
        }
    }
}

#[derive(Default)]
struct State<'t> {
    /// Methods of each class body (keyed by node id), by name; built on first use.
    methods: FxHashMap<usize, FxHashMap<&'t str, Vec<Node<'t>>>>,
    /// Helper methods already known to assert.
    asserting: FxHashSet<usize>,
    /// Helper methods known not to assert (evaluated without cutting a recursion cycle).
    silent: FxHashSet<usize>,
    /// Set while evaluating a helper when a recursion cycle was cut short.
    incomplete: bool,
}

impl<'t> State<'t> {
    /// The methods named `name` declared directly in the class body `body`.
    fn methods_named(&mut self, body: Node<'t>, name: &str, ctx: &Ctx<'t>) -> Option<&[Node<'t>]> {
        let index = self.methods.entry(body.id()).or_insert_with(|| {
            let mut map: FxHashMap<&'t str, Vec<Node<'t>>> = FxHashMap::default();
            for m in ctx
                .members(body)
                .filter(|m| kind(*m) == "method_declaration")
            {
                if let Some(n) = util::name_field(m) {
                    map.entry(ctx.text(n)).or_default().push(m);
                }
            }
            map
        });
        index.get(name).map(Vec::as_slice)
    }
}

impl<'t> JavaRule<'t> for TestWithoutAssertion<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !ctx.is_test() {
            return;
        }
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        let Some(test) = util::test_annotation(ctx, node, &mut self.test_names) else {
            return;
        };
        let expects_exception = test
            .field(Field::Arguments)
            .into_iter()
            .flat_map(named_children)
            .any(|p| {
                kind(p) == "element_value_pair"
                    && p.field(Field::Key)
                        .is_some_and(|k| ctx.text(k).starts_with("expected"))
            });
        if expects_exception || util::is_disabled_test(ctx, node) {
            return;
        }
        // The enclosing classes (JUnit 5 `@Nested` tests use helpers of the outer class).
        let mut class_bodies = Vec::new();
        let mut has_super = false;
        let mut prev: Option<Node<'t>> = None;
        for a in ctx.ancestors(node) {
            let is_type = syntax::is_type_declaration(a)
                || (kind(a) == "object_creation_expression"
                    && prev.is_some_and(|p| kind(p) == "class_body"));
            if is_type {
                has_super |=
                    a.field(Field::Superclass).is_some() || kind(a) == "object_creation_expression";
                if let Some(b) = syntax::type_body(a).or(prev) {
                    class_bodies.push(b);
                }
            }
            prev = Some(a);
        }
        self.imports.init(ctx);
        let mut scope = Scope {
            class_bodies,
            has_super,
            open_static_imports: self.imports.open,
            helper_imports: &self.imports.helpers,
            extended: false,
        };
        let mut visited: Vec<usize> = vec![node.id()];
        if asserts(body, ctx, &scope, &mut self.state, 0, &mut visited) {
            return;
        }
        // Only for a test that seems to assert nothing: the costlier checks (tester objects,
        // lambdas stored in fields). Helpers found silent without them are looked at again.
        scope.extended = true;
        self.state.silent.clear();
        self.state.incomplete = false;
        let mut visited: Vec<usize> = vec![node.id()];
        if asserts(body, ctx, &scope, &mut self.state, 0, &mut visited) {
            return;
        }
        let Some(name) = ctx.name_node(node) else {
            return;
        };
        let text = ctx.text(name);
        ctx.emit(
            name,
            format!("Add an assertion or a verification to the test `{text}`"),
        );
    }
}

struct Scope<'s, 't> {
    /// Bodies of the enclosing classes, innermost first.
    class_bodies: Vec<Node<'t>>,
    /// The test class extends another class (whose helpers may assert).
    has_super: bool,
    /// Static imports that may bring in project-specific assertion helpers.
    open_static_imports: bool,
    /// Single static imports of project-specific methods.
    helper_imports: &'s FxHashSet<&'t str>,
    /// Second pass: also follow tester objects and lambdas stored in fields.
    extended: bool,
}

fn is_assertion_name(name: &str) -> bool {
    const PREFIXES: &[&str] = &["assert", "verify", "expect", "fail", "should", "then"];
    // Mockito stubbing, not verification.
    const STUBBING: &[&str] = &[
        "thenReturn",
        "thenThrow",
        "thenAnswer",
        "thenCallRealMethod",
    ];
    if STUBBING.contains(&name) {
        return false;
    }
    PREFIXES.iter().any(|p| name.starts_with(p))
        || name.contains("Assert")
        || name.contains("Verify")
        || name.contains("Expect")
}

fn asserts<'t>(
    body: Node<'t>,
    ctx: &Ctx<'t>,
    scope: &Scope<'_, 't>,
    state: &mut State<'t>,
    depth: u32,
    visited: &mut Vec<usize>,
) -> bool {
    let mut is_assertion = |n: Node<'t>| match kind(n) {
        "assert_statement" | "throw_statement" => true,
        "method_reference" => {
            let t = ctx.text(n);
            let method = t.rsplit("::").next().unwrap_or("");
            is_assertion_name(method)
        }
        "method_invocation" => call_asserts(n, ctx, scope, state, depth, visited),
        _ => false,
    };
    // Assertions usually come last: search the statements from the end, so that a typical
    // arrange/act/assert test is decided after looking at its final statements only.
    let statements: Vec<Node<'t>> = if kind(body) == "block" {
        named_children(body).collect()
    } else {
        vec![body]
    };
    statements
        .into_iter()
        .rev()
        .any(|s| is_assertion(s) || util::any_descendant(s, |_| false, &mut is_assertion))
}

fn call_asserts<'t>(
    call: Node<'t>,
    ctx: &Ctx<'t>,
    scope: &Scope<'_, 't>,
    state: &mut State<'t>,
    depth: u32,
    visited: &mut Vec<usize>,
) -> bool {
    let Some(name) = util::name_field(call) else {
        return false;
    };
    let method = ctx.text(name);
    if is_assertion_name(method) {
        return true;
    }
    if scope.extended
        && (on_tester_object(ctx, call)
            || lambda_argument_asserts(call, ctx, scope, state, depth, visited))
    {
        return true;
    }
    let unqualified = match util::object_field(call) {
        None => true,
        Some(o) => matches!(kind(o), "this" | "super"),
    };
    if !unqualified {
        return false;
    }
    for body in &scope.class_bodies {
        // Copy the (usually single) overloads out of the index: following them needs `state`.
        let helpers: Vec<Node<'t>> = match state.methods_named(*body, method, ctx) {
            Some(found) => found.to_vec(),
            None => continue,
        };
        for m in helpers {
            let id = m.id();
            if state.asserting.contains(&id) {
                return true;
            }
            if state.silent.contains(&id) {
                continue;
            }
            if visited.contains(&id) {
                state.incomplete = true; // a cycle: this path proves nothing
                continue;
            }
            if depth >= MAX_DEPTH {
                return true; // too deep to follow: assume it asserts
            }
            visited.push(id);
            let Some(mb) = m.field(Field::Body) else {
                return true; // abstract helper: implemented elsewhere
            };
            let outer_incomplete = std::mem::replace(&mut state.incomplete, false);
            let found = asserts(mb, ctx, scope, state, depth + 1, visited);
            if found {
                state.asserting.insert(id);
            } else if !state.incomplete {
                state.silent.insert(id);
            }
            state.incomplete |= outer_incomplete;
            if found {
                return true;
            }
        }
        return false;
    }
    // Possibly an inherited or statically imported helper: assume it asserts.
    scope.has_super || scope.open_static_imports || scope.helper_imports.contains(method)
}

/// Simple type names of assertion helper objects: their methods check what they are given.
fn is_tester_type(name: &str) -> bool {
    const SUFFIXES: &[&str] = &["Tester", "Verifier", "Assert", "Asserts", "Assertions"];
    SUFFIXES
        .iter()
        .any(|s| name.len() > s.len() && name.ends_with(s))
}

/// Whether the call chain starts at a tester class (`Type4Tester.create(..).pop(1)`) or at a
/// variable declared with a tester type (`tester.pop(1)`).
fn on_tester_object<'t>(ctx: &Ctx<'t>, call: Node<'t>) -> bool {
    let mut cur = call;
    for _ in 0..32 {
        let next = match kind(cur) {
            "method_invocation" | "field_access" => util::object_field(cur),
            _ => None,
        };
        match next {
            Some(n) => cur = syntax::strip_parens(n),
            None => break,
        }
    }
    if cur.id() == call.id() || kind(cur) != "identifier" {
        return false;
    }
    let name = ctx.text(cur);
    if name.starts_with(|c: char| c.is_ascii_uppercase()) {
        return is_tester_type(name);
    }
    ctx.declaration_of(cur)
        .and_then(|d| d.type_node)
        .is_some_and(|t| {
            let text = ctx.text(t);
            let base = text.split('<').next().unwrap_or(text);
            is_tester_type(base.rsplit('.').next().unwrap_or(base).trim())
        })
}

/// Whether an argument of the call is a field of this file holding a lambda that asserts
/// (`withArchive(name, CHECK_ENTRIES)` with `static final Check CHECK_ENTRIES = (f, z) -> …`):
/// the helper it is handed to runs it.
fn lambda_argument_asserts<'t>(
    call: Node<'t>,
    ctx: &Ctx<'t>,
    scope: &Scope<'_, 't>,
    state: &mut State<'t>,
    depth: u32,
    visited: &mut Vec<usize>,
) -> bool {
    let Some(args) = call.field(Field::Arguments) else {
        return false;
    };
    for a in named_children(args) {
        if kind(a) != "identifier" {
            continue;
        }
        let Some(decl) = ctx.declaration_of(a) else {
            continue;
        };
        if decl.kind != VarKind::Field {
            continue;
        }
        let Some(value) = decl.value.map(syntax::strip_parens) else {
            continue;
        };
        if kind(value) != "lambda_expression" || visited.contains(&value.id()) {
            continue;
        }
        let Some(body) = value.field(Field::Body) else {
            continue;
        };
        visited.push(value.id());
        if asserts(body, ctx, scope, state, depth + 1, visited) {
            return true;
        }
    }
    false
}

/// Test and JDK libraries whose non-assertion methods (`mock`, `when`, `any`, …) are known not
/// to assert.
fn is_library(path: &str) -> bool {
    const LIBS: &[&str] = &[
        "java.",
        "javax.",
        "org.junit.",
        "org.mockito.",
        "org.assertj.",
        "org.hamcrest.",
        "org.testng.",
        "org.easymock.",
    ];
    LIBS.iter().any(|l| path.starts_with(l))
}
