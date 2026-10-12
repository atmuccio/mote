//! `java/test-could-be-parameterized` — JUnit 5 test methods of one class whose bodies differ
//! only in literal values.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["tests", "duplication"],
    params: &[ParamSpec {
        name: "min_tests",
        default: ParamDefault::Int(3),
        description: "Fewest tests differing only in literals that are reported as a group.",
    }],
    remediation_minutes: 15,
    description: r#"Detects JUnit 5 `@Test` methods of one class whose bodies are the same token
for token except for literal values (numbers, strings, characters, booleans, `null`), when at
least three of them (`min_tests`) share the shape.

**Why it matters.** Copies of one test that differ in their inputs and expected values are a
table written as code: a change to the scenario must be repeated in every copy, the copies
drift apart, and the reader has to diff them to find the cases. A parameterized test states
the scenario once and the cases as data, and every case is still reported on its own.

**How to fix.** Write one `@ParameterizedTest` with the varying literals as parameters, fed by
`@ValueSource`, `@CsvSource` or `@MethodSource`.

```java
// Noncompliant
@Test void parsesOne()   { assertEquals(1, Numbers.parse("one")); }
@Test void parsesTwo()   { assertEquals(2, Numbers.parse("two")); }
@Test void parsesThree() { assertEquals(3, Numbers.parse("three")); }

// Compliant
@ParameterizedTest
@CsvSource({"1, one", "2, two", "3, three"})
void parses(int expected, String text) { assertEquals(expected, Numbers.parse(text)); }
```

A literal only matches a literal of the same kind (`1` and `1L` differ, as they may select
different overloads). Not reported: tests whose parameters, `throws` clauses or annotations
other than `@Test` and `@DisplayName` differ (`@Disabled`, `@Tag`, `@Timeout`, …), identical
copies (see `java/identical-methods`), JUnit 4 and TestNG tests (their parameterization works
per class), and methods that are already parameterized."#,
    ..RuleMeta::new(
        "java/test-could-be-parameterized",
        "java",
        "Tests differing only in literals could be one parameterized test",
    )
};

#[derive(Default)]
pub struct TestCouldBeParameterized {
    /// Whether the file imports JUnit Jupiter's `@Test` (computed on first use).
    jupiter: Option<bool>,
}

/// Tokens lexed for the first grouping of tests (see `check`).
const PREFIX_TOKENS: usize = 64;

/// A test method's body reduced to its token shape, plus the literal values.
struct Shape<'t> {
    /// The whole body was lexed (not only the first `PREFIX_TOKENS` tokens).
    complete: bool,
    method: Node<'t>,
    /// Hash of the signature parts that must agree and of the tokens, literals replaced by
    /// their kind.
    key: ShapeKey,
    literals: Vec<&'t str>,
}

fn is_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$' || c >= 0x80
}

/// The literal kind of a number's source (`1`, `1L`, `1.5f`, `0x1F`, `1e-3`).
fn number_kind(t: &[u8], hex: bool) -> u8 {
    let last = t.last().map_or(b'0', u8::to_ascii_lowercase);
    let has = |c: u8| t.iter().any(|x| x.to_ascii_lowercase() == c);
    if last == b'l' {
        b'L'
    } else if !hex && last == b'f' {
        b'F'
    } else if has(b'.') || (hex && has(b'p')) || (!hex && (has(b'e') || last == b'd')) {
        b'D'
    } else {
        b'I'
    }
}

/// Two independent 64-bit hashes of a token stream: equal streams give equal keys, and a
/// collision of both is negligible.
#[derive(Clone, Copy, PartialEq, Eq)]
struct ShapeKey(u64, u64);

impl ShapeKey {
    fn new() -> Self {
        ShapeKey(0xcbf2_9ce4_8422_2325, 0)
    }

    /// Mixes one token (eight bytes at a time, then the rest with its length).
    fn feed(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for c in &mut chunks {
            let mut w = [0u8; 8];
            w.copy_from_slice(c);
            self.mix(u64::from_le_bytes(w));
        }
        let rest = chunks.remainder();
        let mut w = [0u8; 8];
        w[..rest.len()].copy_from_slice(rest);
        self.mix(u64::from_le_bytes(w) ^ ((rest.len() as u64) << 59));
    }

    fn mix(&mut self, w: u64) {
        self.0 = (self.0 ^ w).wrapping_mul(0x0000_0100_0000_01b3);
        self.1 = (self.1.rotate_left(5) ^ w).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}

/// Emits a literal: its kind into the token stream, its text into `literals`.
fn literal<'t>(
    emit: &mut impl FnMut(&[u8]),
    literals: &mut Vec<&'t str>,
    text: &'t str,
    kind: u8,
    start: usize,
    end: usize,
) {
    emit(&[1, kind]);
    literals.push(text.get(start..end).unwrap_or(""));
}

/// Emits the token shape of the body source `text`: comments and layout dropped, literals
/// (strings, text blocks, characters, numbers, booleans, `null`) replaced by their kind and
/// collected in `literals`. A lexical scan: much cheaper than visiting every token of the
/// syntax tree. `None` on an unterminated literal.
fn lex_shape<'t>(
    text: &'t str,
    emit: &mut impl FnMut(&[u8]),
    literals: &mut Vec<&'t str>,
    max_tokens: usize,
) -> Option<bool> {
    let b = text.as_bytes();
    let mut i = 0;
    let mut last_word = false;
    let mut tokens = 0usize;
    while i < b.len() {
        let c = b[i];
        if !matches!(c, b' ' | b'\t' | b'\r' | b'\n' | 0x0c | b'/') {
            if tokens >= max_tokens {
                return Some(false);
            }
            tokens += 1;
        }
        match c {
            b' ' | b'\t' | b'\r' | b'\n' | 0x0c => {
                i += 1;
            }
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i < b.len() && !(b[i] == b'*' && b.get(i + 1) == Some(&b'/')) {
                    i += 1;
                }
                i += 2;
            }
            b'"' => {
                let start = i;
                let block = b.get(i + 1) == Some(&b'"') && b.get(i + 2) == Some(&b'"');
                i += if block { 3 } else { 1 };
                loop {
                    let d = *b.get(i)?;
                    if d == b'\\' {
                        i += 2;
                        continue;
                    }
                    if block {
                        if d == b'"' && b.get(i + 1) == Some(&b'"') && b.get(i + 2) == Some(&b'"') {
                            i += 3;
                            break;
                        }
                    } else if d == b'"' {
                        i += 1;
                        break;
                    } else if d == b'\n' {
                        return None;
                    }
                    i += 1;
                }
                let kind = if block { b'T' } else { b'S' };
                literal(emit, literals, text, kind, start, i.min(b.len()));
                last_word = false;
            }
            b'\'' => {
                let start = i;
                i += 1;
                loop {
                    let d = *b.get(i)?;
                    if d == b'\\' {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    if d == b'\'' {
                        break;
                    }
                    if d == b'\n' {
                        return None;
                    }
                }
                literal(emit, literals, text, b'C', start, i.min(b.len()));
                last_word = false;
            }
            c if c.is_ascii_digit() => {
                let start = i;
                let hex = c == b'0' && matches!(b.get(i + 1), Some(b'x' | b'X'));
                while i < b.len() {
                    let d = b[i];
                    let exp_sign = (d == b'+' || d == b'-')
                        && i > start
                        && if hex {
                            matches!(b[i - 1], b'p' | b'P')
                        } else {
                            matches!(b[i - 1], b'e' | b'E')
                        };
                    if is_ident(d) || d == b'.' || exp_sign {
                        i += 1;
                    } else {
                        break;
                    }
                }
                let kind = number_kind(&b[start..i], hex);
                literal(emit, literals, text, kind, start, i);
                last_word = false;
            }
            c if is_ident(c) => {
                let start = i;
                while i < b.len() && is_ident(b[i]) {
                    i += 1;
                }
                let word = &b[start..i];
                match word {
                    b"true" | b"false" => {
                        literal(emit, literals, text, b'B', start, i);
                        last_word = false;
                    }
                    b"null" => {
                        literal(emit, literals, text, b'N', start, i);
                        last_word = false;
                    }
                    _ => {
                        if last_word {
                            emit(b" ");
                        }
                        emit(word);
                        last_word = true;
                    }
                }
            }
            _ => {
                emit(&[c]);
                last_word = false;
                i += 1;
            }
        }
    }
    Some(true)
}

fn shape<'t>(
    ctx: &Ctx<'t>,
    method: Node<'t>,
    body: Node<'t>,
    sig: &str,
    max_tokens: usize,
) -> Option<Shape<'t>> {
    let mut key = ShapeKey::new();
    key.feed(sig.as_bytes());
    let mut literals = Vec::new();
    let complete = lex_shape(
        ctx.text(body),
        &mut |b: &[u8]| key.feed(b),
        &mut literals,
        max_tokens,
    )?;
    Some(Shape {
        complete,
        method,
        key,
        literals,
    })
}

impl TestCouldBeParameterized {
    fn is_jupiter(&mut self, ctx: &Ctx<'_>) -> bool {
        *self.jupiter.get_or_insert_with(|| {
            ctx.imports().iter().any(|i| {
                !i.is_static
                    && ((!i.is_wildcard && i.path == "org.junit.jupiter.api.Test")
                        || (i.is_wildcard && i.path == "org.junit.jupiter.api"))
            }) && !ctx
                .imports()
                .iter()
                .any(|i| !i.is_static && !i.is_wildcard && i.path == "org.junit.Test")
        })
    }
}

/// The signature parts that must agree for two tests to merge (parameters, `throws`, and the
/// annotations other than `@Test` / `@DisplayName`), and the body. `None` when the method is
/// not a plain test. Reads the `modifiers` node and the source between the name and the body
/// only: this runs for every method of a test class.
fn signature<'t>(ctx: &Ctx<'t>, m: Node<'t>) -> Option<(String, Node<'t>)> {
    let src = ctx.source();
    let mods = syntax::modifiers(m)?;
    if !ctx.text(mods).contains("Test") {
        return None;
    }
    let mut is_test = false;
    let mut anns: Vec<&str> = Vec::new();
    for a in named_children(mods) {
        if !matches!(kind(a), "annotation" | "marker_annotation") {
            continue;
        }
        match syntax::annotation_name(a, src) {
            "Test" => {
                let full = a.field(Field::Name).map(|n| ctx.text(n)).unwrap_or("");
                if full != "Test" && full != "org.junit.jupiter.api.Test" {
                    return None;
                }
                is_test = true;
            }
            "DisplayName" => {}
            "ParameterizedTest" | "RepeatedTest" | "TestFactory" | "TestTemplate" => return None,
            _ => anns.push(ctx.text(a)),
        }
    }
    if !is_test {
        return None;
    }
    anns.sort_unstable();
    // Parameters and `throws`: everything between the name and the body.
    let name = m.field(Field::Name)?;
    let body = m.field(Field::Body)?;
    let tail = src.get(name.end_byte()..body.start_byte())?;
    let mut sig = String::new();
    for a in anns {
        sig.push_str(a);
        sig.push('\u{2}');
    }
    for part in tail.split_whitespace() {
        sig.push_str(part);
        sig.push(' ');
    }
    sig.push('\u{2}');
    Some((sig, body))
}

impl<'t> JavaRule<'t> for TestCouldBeParameterized {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["class_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !self.is_jupiter(ctx) {
            return;
        }
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        let min = ctx.param_i64("min_tests").max(2) as usize;
        // Phase 1: the shape of the first bytes of every test. Copies of one test share it, so
        // only tests whose prefix is shared often enough are lexed to the end.
        let mut firsts: Vec<(String, Shape<'t>, Node<'t>)> = Vec::new();
        for m in ctx.members(body) {
            if kind(m) != "method_declaration" {
                continue;
            }
            let Some((sig, b)) = signature(ctx, m) else {
                continue;
            };
            if let Some(s) = shape(ctx, m, b, &sig, PREFIX_TOKENS) {
                firsts.push((sig, s, b));
            }
        }
        if firsts.len() < min {
            return;
        }
        let mut order: Vec<usize> = (0..firsts.len()).collect();
        order.sort_by_key(|&k| (firsts[k].1.key.0, firsts[k].1.key.1));
        let mut wanted = vec![false; firsts.len()];
        let mut start = 0;
        while start < order.len() {
            let key = firsts[order[start]].1.key;
            let end = start
                + order[start..]
                    .iter()
                    .take_while(|&&k| firsts[k].1.key == key)
                    .count();
            if end - start >= min {
                for &k in &order[start..end] {
                    wanted[k] = true;
                }
            }
            start = end;
        }
        // Phase 2: complete shapes, in source order.
        let mut shapes: Vec<Shape<'t>> = Vec::new();
        for (k, (sig, first, b)) in firsts.into_iter().enumerate() {
            if !wanted[k] {
                continue;
            }
            let full = if first.complete {
                Some(first)
            } else {
                shape(ctx, first.method, b, &sig, usize::MAX)
            };
            if let Some(s) = full.filter(|s| !s.literals.is_empty()) {
                shapes.push(s);
            }
        }
        if shapes.len() < min {
            return;
        }
        let mut done = vec![false; shapes.len()];
        for i in 0..shapes.len() {
            if done[i] {
                continue;
            }
            let group: Vec<usize> = (i..shapes.len())
                .filter(|&j| !done[j] && shapes[j].key == shapes[i].key)
                .collect();
            for &j in &group {
                done[j] = true;
            }
            if group.len() < min {
                continue;
            }
            // Identical copies are another rule's finding; the group must vary somewhere.
            if group
                .iter()
                .all(|&j| shapes[j].literals == shapes[i].literals)
            {
                continue;
            }
            let Some(name) = shapes[i].method.field(Field::Name) else {
                continue;
            };
            let n = group.len();
            let mut names: Vec<&str> = group
                .iter()
                .take(4)
                .filter_map(|&j| shapes[j].method.field(Field::Name).map(|m| ctx.text(m)))
                .collect();
            if n > 4 {
                names.push("…");
            }
            let mut e = ctx
                .emit(
                    name,
                    format!(
                        "Merge these {n} tests ({}) into one @ParameterizedTest: they differ only in literal values",
                        names.join(", ")
                    ),
                )
                .value(n as f64);
            for &j in group.iter().skip(1) {
                if let Some(other) = shapes[j].method.field(Field::Name) {
                    e = e.secondary(other, "same test with other literals");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::lex_shape;

    fn shape(s: &str) -> (Vec<u8>, Vec<&str>) {
        let mut key = Vec::new();
        let mut lits = Vec::new();
        assert!(
            lex_shape(
                s,
                &mut |b: &[u8]| key.extend_from_slice(b),
                &mut lits,
                usize::MAX
            )
            .is_some(),
            "{s}"
        );
        (key, lits)
    }

    #[test]
    fn shapes_ignore_layout_and_collect_literals() {
        let (a, la) = shape("{ assertEquals(1, f(\"one\")); // c\n }");
        let (b, lb) = shape("{assertEquals( 2,f(\"two\")) ; /* d */}");
        assert_eq!(a, b);
        assert_eq!(la, vec!["1", "\"one\""]);
        assert_eq!(lb, vec!["2", "\"two\""]);
        assert_ne!(shape("{f(1);}").0, shape("{f(1L);}").0);
        assert_ne!(shape("{f(1);}").0, shape("{f(1.0);}").0);
        assert_ne!(shape("{f(1.0f);}").0, shape("{f(1.0);}").0);
        assert_eq!(shape("{f(1e-5);}").1, vec!["1e-5"]);
        assert_eq!(shape("{f(0x1F);}").1, vec!["0x1F"]);
        assert_eq!(shape("{f(x1);}").1, Vec::<&str>::new());
        assert_eq!(
            shape(r"{f('\'', true, null);}").1,
            vec![r"'\''", "true", "null"]
        );
        assert_eq!(shape("{s = \"\"\"\n a \"\n \"\"\";}").1.len(), 1);
        assert_ne!(shape("{int x;}").0, shape("{intx;}").0);
        let mut l = Vec::new();
        assert!(lex_shape("{f(\"x);\n}", &mut |_: &[u8]| {}, &mut l, usize::MAX).is_none());
        // A token budget cuts at the same token whatever the literals' lengths.
        let mut a = Vec::new();
        let mut la = Vec::new();
        let done = lex_shape(
            "{ f(\"a long literal\"); g(); }",
            &mut |b: &[u8]| a.extend_from_slice(b),
            &mut la,
            5,
        );
        let mut b = Vec::new();
        let mut lb = Vec::new();
        let done_b = lex_shape(
            "{ f(\"x\"); /* c */ g(); }",
            &mut |x: &[u8]| b.extend_from_slice(x),
            &mut lb,
            5,
        );
        assert_eq!((done, done_b), (Some(false), Some(false)));
        assert_eq!(a, b);
    }
}
