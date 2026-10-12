//! `java/nondeterministic-test` — test code drawing data from an unseeded random source, or
//! marked to be retried until it passes.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["tests", "flaky"],
    remediation_minutes: 10,
    description: r#"Detects test code whose outcome can change from one run to the next:

* data drawn from an unseeded random source: `new Random()`, `Math.random()`,
  `ThreadLocalRandom.current()`, `Collections.shuffle(list)` without a `Random`, and the
  Commons Lang `RandomStringUtils` / `RandomUtils` helpers;
* tests marked to be run again until they pass: JUnit Pioneer's `@RetryingTest`,
  `@RepeatedIfExceptionsTest`, TestNG's `retryAnalyzer`.

**Why it matters.** A test is only useful if a failure can be reproduced. With a fresh random
seed on every run, a failing input shows up once on a build server and cannot be replayed
while debugging; the test becomes "flaky" and the team learns to re-run it instead of reading
it. Retrying until green is the same problem made official: a real intermittent bug (a race,
an ordering assumption) is hidden behind the retries.

**How to fix.** Use a fixed seed (`new Random(42)`), or a random seed that the test logs and
accepts as a parameter so a failure can be replayed; make the flaky behavior deterministic
instead of retrying.

```java
// Noncompliant
@Test
void sortsAnyInput() {
    int[] data = new Random().ints(100).toArray();
    …
}

// Compliant
@Test
void sortsAnyInput() {
    int[] data = new Random(20260928L).ints(100).toArray();
    …
}
```

Only test files are checked. Not reported: seeded generators (also when `setSeed` follows,
or when an unseeded generator only draws the seed, `long seed = new Random().nextLong()`, the
usual start of a replayable test), `SecureRandom` (used for keys
and tokens, not for test data), `UUID.randomUUID()` (unique names, not data a result depends
on), random values only used to build a unique name for a file, table or queue or to pick a
network port (`int listenPort = RandomUtils.nextInt(10000, 20000)`, a `createRandomPort()`
helper: a clash is fixed by a port the system assigns, not by a seed), and generators stored
in a variable that nothing reads (no data is drawn from them)."#,
    ..RuleMeta::new(
        "java/nondeterministic-test",
        "java",
        "Test depends on randomness or retries",
    )
};

#[derive(Default)]
pub struct NondeterministicTest {
    gate: util::Gate,
}

/// Retry annotations (FQN).
const RETRY_ANNOTATIONS: &[&str] = &[
    "org.junitpioneer.jupiter.RetryingTest",
    "io.github.artsok.RepeatedIfExceptionsTest",
];

/// Whether the random value built at `node` only goes into a string concatenation (a unique
/// suffix for a name): the call is an operand of a `+` string concatenation, directly or
/// through `nextInt()` / `nextLong()` on the new generator.
fn only_names_something<'t>(ctx: &Ctx<'t>, node: Node<'t>) -> bool {
    let mut cur = node;
    for _ in 0..3 {
        let Some(p) = ctx.parent(cur) else {
            return false;
        };
        match kind(p) {
            "method_invocation" if p.field(Field::Object).is_some_and(|o| o.id() == cur.id()) => {
                cur = p;
            }
            "parenthesized_expression" => cur = p,
            "binary_expression" => {
                return syntax::operator_str(p) == "+"
                    && ctx.type_of(p).is_some_and(|t| t.is_string());
            }
            "argument_list" => {
                // `String.valueOf(random.nextInt())`, `Long.toString(..)`.
                return ctx.parent(p).is_some_and(|call| {
                    matches!(
                        ctx.call_name(call),
                        Some("valueOf" | "toString" | "toHexString")
                    )
                });
            }
            _ => return false,
        }
    }
    false
}

/// Number of whole-word occurrences of `name` in `text`.
fn word_count(text: &str, name: &str) -> usize {
    let is_word = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    text.match_indices(name)
        .filter(|(i, _)| {
            !text[..*i].chars().next_back().is_some_and(is_word)
                && !text[i + name.len()..].chars().next().is_some_and(is_word)
        })
        .count()
}

/// Whether one of the camel-case or snake-case words of `ident` is `word` (lower case),
/// ignoring case and a plural `s`: `listenPort`, `SERVER_PORT`, `createRandomPort` for `port`,
/// but not `report` or `transport`.
fn has_word(ident: &str, word: &str) -> bool {
    let chars: Vec<char> = ident.chars().collect();
    let mut start = 0;
    let mut words: Vec<String> = Vec::new();
    for i in 1..=chars.len() {
        let boundary = i == chars.len()
            || !chars[i].is_alphanumeric()
            || (chars[i].is_uppercase() && chars[i - 1].is_lowercase())
            || (chars[i].is_uppercase()
                && chars[i - 1].is_uppercase()
                && chars.get(i + 1).is_some_and(|c| c.is_lowercase()));
        if boundary {
            let w: String = chars[start..i]
                .iter()
                .filter(|c| c.is_alphanumeric())
                .collect();
            if !w.is_empty() {
                words.push(w.to_lowercase());
            }
            start = i;
        }
    }
    words
        .iter()
        .any(|w| w == word || w.strip_suffix('s') == Some(word))
}

/// A generator stored in a variable that nothing reads: no draw depends on it. A local counts
/// when its name appears nowhere else in its function; a field when it appears nowhere else in
/// its class body (a non-private field of an abstract class may serve subclasses elsewhere).
fn unused_generator<'t>(ctx: &Ctx<'t>, node: Node<'t>) -> bool {
    let Some(decl) = ctx
        .parent(node)
        .filter(|p| kind(*p) == "variable_declarator")
    else {
        return false;
    };
    if decl.field(Field::Value).map(|v| v.id()) != Some(node.id()) {
        return false;
    }
    let Some(name) = decl.field(Field::Name).map(|n| ctx.text(n)) else {
        return false;
    };
    let Some(stmt) = ctx.parent(decl) else {
        return false;
    };
    let scope = match kind(stmt) {
        "local_variable_declaration" => ctx.enclosing_function(node),
        "field_declaration" => {
            let Some(class) = ctx.enclosing_class(node) else {
                return false;
            };
            if syntax::has_modifier(class, "abstract") && !syntax::has_modifier(stmt, "private") {
                return false;
            }
            if kind(class) == "interface_declaration" {
                return false;
            }
            syntax::type_body(class).or(Some(class))
        }
        _ => None,
    };
    scope.is_some_and(|s| word_count(ctx.text(s), name) <= 1)
}

/// A random value that only picks a network port (a resource identity like the unique names
/// above; the fix for clashes is a port the system assigns, not a seed): drawn inside a method
/// returning an integer whose name says it makes a port (`createRandomPort()`), or flowing
/// (through the draw call, arithmetic, casts) into a variable, assignment or call named after a
/// port (`int listenPort = …`, `server.setPort(…)`).
fn picks_port<'t>(ctx: &Ctx<'t>, node: Node<'t>) -> bool {
    if let Some(m) = ctx
        .enclosing_method(node)
        .filter(|m| kind(*m) == "method_declaration")
    {
        let returns_int = m
            .field(Field::Type)
            .is_some_and(|t| matches!(ctx.text(t), "int" | "Integer" | "long" | "Long"));
        if returns_int && has_word(syntax::name_of(m, ctx.source()), "port") {
            return true;
        }
    }
    let mut cur = node;
    for _ in 0..8 {
        let Some(p) = ctx.parent(cur) else {
            return false;
        };
        match kind(p) {
            "method_invocation" if p.field(Field::Object).is_some_and(|o| o.id() == cur.id()) => {
                cur = p;
            }
            "parenthesized_expression" | "cast_expression" => cur = p,
            "binary_expression" if matches!(syntax::operator_str(p), "+" | "-" | "*" | "%") => {
                cur = p;
            }
            "variable_declarator" => {
                return p
                    .field(Field::Name)
                    .is_some_and(|n| has_word(ctx.text(n), "port"));
            }
            "assignment_expression" => {
                return p.field(Field::Left).is_some_and(|l| {
                    let target = match kind(l) {
                        "field_access" => l.field(Field::Field).unwrap_or(l),
                        _ => l,
                    };
                    has_word(ctx.text(target), "port")
                });
            }
            "argument_list" => {
                return ctx
                    .parent(p)
                    .and_then(|call| ctx.call_name(call))
                    .is_some_and(|n| has_word(n, "port"));
            }
            _ => return false,
        }
    }
    false
}

/// `new Random()` that only produces a seed (`long seed = new Random().nextLong();`, kept so
/// a failing run can be replayed) or that is seeded right after (`r.setSeed(42)`).
fn seeds_something<'t>(ctx: &Ctx<'t>, creation: Node<'t>) -> bool {
    let Some(parent) = ctx.parent(creation) else {
        return false;
    };
    // `new Random().nextLong()` into a variable named like a seed.
    if kind(parent) == "method_invocation"
        && parent
            .field(Field::Object)
            .is_some_and(|o| o.id() == creation.id())
    {
        let target = ctx.parent(parent).and_then(|p| match kind(p) {
            "variable_declarator" => p.field(Field::Name),
            "assignment_expression" => p.field(Field::Left),
            _ => None,
        });
        return target.is_some_and(|t| ctx.text(t).to_ascii_lowercase().contains("seed"));
    }
    // `Random r = new Random(); r.setSeed(..)`.
    if kind(parent) == "variable_declarator" {
        let Some(name) = parent.field(Field::Name) else {
            return false;
        };
        let needle = format!("{}.setSeed(", ctx.text(name));
        return ctx
            .enclosing_function(creation)
            .or_else(|| ctx.enclosing_class(creation))
            .is_some_and(|f| ctx.text(f).contains(&needle));
    }
    false
}

impl<'t> JavaRule<'t> for NondeterministicTest {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "object_creation_expression",
            "method_invocation",
            "marker_annotation",
            "annotation",
        ]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &[
            "random",
            "current",
            "shuffle",
            "randomAlphabetic",
            "randomAlphanumeric",
            "randomNumeric",
            "randomAscii",
            "randomPrint",
            "randomGraph",
            "nextInt",
            "nextLong",
            "nextDouble",
            "nextFloat",
            "nextBoolean",
            "nextBytes",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !ctx.is_test() {
            return;
        }
        match kind(node) {
            "object_creation_expression" => {
                if util::arg_count(node) != 0 || syntax::child_of_kind(node, "class_body").is_some()
                {
                    return;
                }
                let Some(ty) = node.field(Field::Type) else {
                    return;
                };
                if ctx.text(ty) != "Random" && ctx.text(ty) != "java.util.Random" {
                    return;
                }
                if util::type_fqn(ctx, ty).as_deref() != Some("java.util.Random")
                    || only_names_something(ctx, node)
                    || seeds_something(ctx, node)
                    || unused_generator(ctx, node)
                    || picks_port(ctx, node)
                {
                    return;
                }
                ctx.emit(
                    node,
                    "Seed this random generator (or log its seed) so that a failing run can be \
                     reproduced",
                );
            }
            "method_invocation" => {
                let Some(name) = ctx.call_name(node) else {
                    return;
                };
                let Some(recv) = node.field(Field::Object) else {
                    return;
                };
                let n = util::arg_count(node);
                let owner = match (name, n) {
                    ("random", 0) => "java.lang.Math",
                    ("current", 0) => "java.util.concurrent.ThreadLocalRandom",
                    ("shuffle", 1) => "java.util.Collections",
                    (
                        "randomAlphabetic" | "randomAlphanumeric" | "randomNumeric" | "randomAscii"
                        | "randomPrint" | "randomGraph" | "random",
                        _,
                    ) => "org.apache.commons.lang3.RandomStringUtils",
                    (
                        "nextInt" | "nextLong" | "nextDouble" | "nextFloat" | "nextBoolean"
                        | "nextBytes",
                        _,
                    ) => "org.apache.commons.lang3.RandomUtils",
                    _ => return,
                };
                if !util::names_type(ctx, recv, owner)
                    || only_names_something(ctx, node)
                    // `current()` hands out a generator; the other calls draw a value.
                    || (name == "current" && unused_generator(ctx, node))
                    || picks_port(ctx, node)
                {
                    return;
                }
                let what = util::last_segment(owner);
                ctx.emit(
                    node,
                    format!(
                        "`{what}.{name}` draws from an unseeded random source: use a seeded \
                         `Random` so that a failing run can be reproduced"
                    ),
                );
            }
            _ => {
                // Retry annotations.
                let Some(name) = node.field(Field::Name) else {
                    return;
                };
                let text = util::last_segment(ctx.text(name));
                let retry = match text {
                    "RetryingTest" | "RepeatedIfExceptionsTest" => {
                        util::annotation_is(ctx, node, RETRY_ANNOTATIONS)
                    }
                    "Test" => {
                        kind(node) == "annotation"
                            && ctx.text(node).contains("retryAnalyzer")
                            && self.gate.open(ctx, "org.testng")
                    }
                    _ => false,
                };
                if retry {
                    ctx.emit(
                        node,
                        "Make this test deterministic instead of retrying it until it passes",
                    );
                }
            }
        }
    }
}
