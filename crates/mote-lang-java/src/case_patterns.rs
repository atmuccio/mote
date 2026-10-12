//! Pre-parse workaround for a grammar gap: tree-sitter-java 0.23 accepts neither annotations nor
//! `final` before the type of a `case` type pattern (`case @SuppressWarnings("unused") Foo f ->`,
//! `case final Foo f ->`, valid since Java 21) and recovers with errors that cascade over the
//! rest of the switch.
//!
//! Before parsing, those modifiers are replaced by spaces (newlines kept), as the varargs
//! annotations are (see `varargs`), so the grammar sees `case     Foo f ->`. Every byte keeps
//! its offset, line and column: nodes, findings and the text rules read (always taken from the
//! original source) are unchanged. Only modifiers that directly follow the `case` keyword in code
//! (not in comments, strings or text blocks) and precede a type are touched; a file without
//! `case` followed by `@` or `final` is not even scanned.
//!
//! Not handled (no occurrence in the corpora): modifiers on the components of a record pattern
//! (`case Point(final int x, int y)`), which the grammar rejects too.

use std::borrow::Cow;

fn is_ident_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$' || c >= 0x80
}

fn is_ident_start(c: u8) -> bool {
    is_ident_byte(c) && !c.is_ascii_digit()
}

fn skip_space(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

/// The end of the identifier-like word starting at `i` (`i` itself when there is none).
fn word_end(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && is_ident_byte(b[i]) {
        i += 1;
    }
    i
}

/// Whether some `case` is followed (after whitespace) by `@` or the word `final`.
fn may_have_modifiers(text: &str) -> bool {
    let b = text.as_bytes();
    text.match_indices("case").any(|(at, _)| {
        let i = skip_space(b, at + 4);
        b.get(i) == Some(&b'@') || (b[i..].starts_with(b"final") && word_end(b, i) == i + 5)
    })
}

/// The end of the string literal, character literal or text block opening at `i` (a `"` or a
/// `'`), and whether it was closed. Ordinary literals stop at the end of the line.
fn skip_literal(b: &[u8], i: usize) -> (usize, bool) {
    let quote = b[i];
    if quote == b'"' && b[i..].starts_with(b"\"\"\"") {
        let mut j = i + 3;
        while j < b.len() {
            if b[j] == b'\\' {
                j += 2;
            } else if b[j..].starts_with(b"\"\"\"") {
                return (j + 3, true);
            } else {
                j += 1;
            }
        }
        return (b.len(), false);
    }
    let mut j = i + 1;
    while j < b.len() {
        match b[j] {
            b'\\' => j += 2,
            b'\n' | b'\r' => return (j, false),
            c if c == quote => return (j + 1, true),
            _ => j += 1,
        }
    }
    (b.len(), false)
}

/// The end of the annotation starting at `at` (`@Name`, `@a.b.Name`, `@Name(args)`), or `None`
/// when it is not understood (a comment or an unclosed literal in its arguments, `@interface`).
fn annotation_end(b: &[u8], at: usize) -> Option<usize> {
    let mut i = at + 1;
    loop {
        if !b.get(i).copied().is_some_and(is_ident_start) {
            return None;
        }
        let start = i;
        i = word_end(b, i);
        if &b[start..i] == b"interface" {
            return None;
        }
        if b.get(i) == Some(&b'.') {
            i += 1;
            continue;
        }
        break;
    }
    let open = skip_space(b, i);
    if b.get(open) != Some(&b'(') {
        return Some(i);
    }
    let mut depth = 0usize;
    let mut j = open;
    while j < b.len() {
        match b[j] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(j + 1);
                }
            }
            b'"' | b'\'' => {
                let (end, closed) = skip_literal(b, j);
                if !closed {
                    return None;
                }
                j = end;
                continue;
            }
            b'/' if matches!(b.get(j + 1), Some(b'/' | b'*')) => return None,
            b';' => return None,
            _ => {}
        }
        j += 1;
    }
    None
}

/// The modifiers (annotations, `final`) after a `case` keyword ending at `i`, when they are
/// followed by the start of a type; `None` otherwise or when an annotation is not understood.
fn case_modifiers(b: &[u8], mut i: usize) -> Option<Vec<(usize, usize)>> {
    let mut found = Vec::new();
    loop {
        let j = skip_space(b, i);
        if b.get(j) == Some(&b'@') {
            let end = annotation_end(b, j)?;
            found.push((j, end));
            i = end;
        } else if b[j..].starts_with(b"final") && word_end(b, j) == j + 5 {
            found.push((j, j + 5));
            i = j + 5;
        } else {
            let type_follows = b.get(j).copied().is_some_and(is_ident_start);
            return (!found.is_empty() && type_follows).then_some(found);
        }
    }
}

/// The spans of every pattern modifier after a `case` keyword of the code (comments, string and
/// character literals and text blocks are skipped).
fn modifier_spans(b: &[u8]) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i < b.len() && !b[i..].starts_with(b"*/") {
                    i += 1;
                }
                i += 2;
            }
            b'"' | b'\'' => i = skip_literal(b, i).0,
            c if is_ident_byte(c) => {
                let start = i;
                i = word_end(b, i);
                if &b[start..i] == b"case" {
                    if let Some(found) = case_modifiers(b, i) {
                        if let Some(&(_, end)) = found.last() {
                            i = end;
                        }
                        spans.extend(found);
                    }
                }
            }
            _ => i += 1,
        }
    }
    spans
}

/// `text` with every annotation and `final` of a `case` type pattern blanked out, or `text`
/// itself when there is none.
pub(crate) fn blank_case_pattern_modifiers(text: Cow<'_, str>) -> Cow<'_, str> {
    if !may_have_modifiers(&text) {
        return text;
    }
    let spans = modifier_spans(text.as_bytes());
    if spans.is_empty() {
        return text;
    }
    let mut out = text.as_bytes().to_vec();
    for (start, end) in spans {
        for c in &mut out[start..end] {
            if *c != b'\n' && *c != b'\r' {
                *c = b' ';
            }
        }
    }
    // Spans start and end on ASCII bytes and are blanked whole, so the result stays UTF-8.
    match String::from_utf8(out) {
        Ok(s) => Cow::Owned(s),
        Err(_) => text,
    }
}

#[cfg(test)]
mod tests {
    use super::blank_case_pattern_modifiers;
    use std::borrow::Cow;

    fn blank(src: &str) -> Cow<'_, str> {
        blank_case_pattern_modifiers(Cow::Borrowed(src))
    }

    /// `n` spaces.
    fn sp(n: usize) -> String {
        " ".repeat(n)
    }

    #[test]
    fn blanks_annotations_and_final_of_case_patterns() {
        let src = "case @SuppressWarnings(\"unused\") Foo f -> 1;";
        let out = blank(src);
        assert_eq!(out.len(), src.len());
        assert_eq!(out, format!("case{}Foo f -> 1;", sp(29)));
        assert_eq!(blank("case final Foo f:"), format!("case{}Foo f:", sp(7)));
        // Several modifiers, qualified names, array arguments, a `)` and an escaped quote in a
        // string argument.
        let src = "case final @a.b.C @D({\"x\", \"y)\\\"\"}) Map.Entry<K, V> e ->";
        assert_eq!(blank(src), format!("case{}Map.Entry<K, V> e ->", sp(32)));
        // Newlines are kept, so every later line keeps its number.
        assert_eq!(
            blank("case @SuppressWarnings(\n  \"unused\")\n  Foo f ->"),
            format!("case{}\n{}\n  Foo f ->", sp(19), sp(11))
        );
        // A non-ASCII string argument is blanked whole (every byte of it).
        let src = "case @A(\"é\") B b ->";
        assert_eq!(blank(src), format!("case{}B b ->", sp(src.len() - 10)));
        // Every label of a switch.
        assert_eq!(
            blank("switch (o) { case @A X x -> 1; case Y y -> 2; case final Z z -> 3; }"),
            format!(
                "switch (o) {{ case{}X x -> 1; case Y y -> 2; case{}Z z -> 3; }}",
                sp(4),
                sp(7)
            )
        );
    }

    #[test]
    fn leaves_other_code_alone() {
        for src in [
            "switch (o) { case Foo f -> 1; case 1 -> 2; default -> 3; }",
            "// in that case @param x is final\nclass A {}",
            "/* case @A X x -> */ class A {}",
            "class A { String s = \"case @A X x\"; char c = '@'; }",
            "class A { String s = \"\"\"\n  case final X x\n  \"\"\"; }",
            "class A { void f() { showcase @A; lowercase final; } }",
            "class A { void f() { switch (k) { case FINAL -> 1; case finally_ -> 2; } } }",
            // Not understood: a comment in the arguments, an unclosed string, nothing after.
            "case @A(/* x */ 1) X x ->",
            "case @A(\"x) X x ->",
            "case @A",
            "case @interface",
            "case final",
        ] {
            assert!(matches!(blank(src), Cow::Borrowed(_)), "{src}");
        }
    }

    #[test]
    fn modified_case_patterns_parse_without_errors_and_keep_positions() {
        use crate::parse::{parse, ParseBudget};
        use mote_core::testing::ruleset_for;
        use mote_core::{LanguagePlugin, SourceFile};
        let src = "class A {
  int f(Object o) {
    switch (o) {
      case @SuppressWarnings(\"unused\") String s -> { return 1; }
      case final Integer i -> { return i; }
      case @SuppressWarnings({\"unused\", \"rawtypes\"})
          final java.util.List l -> { return 2; }
      default -> { }
    }
    try { g(); } catch (RuntimeException e) { }
    return 0;
  }
}
";
        // The grammar alone fails on it.
        let mut p = tree_sitter::Parser::new();
        p.set_language(&crate::kinds::LANGUAGE).unwrap();
        assert!(p.parse(src, None).unwrap().root_node().has_error());
        let tree = parse(src, ParseBudget::DEFAULT, None).expect("tree");
        assert!(!tree.root_node().has_error());
        let plugin = crate::JavaPlugin::new();
        let metas = plugin.rules();
        let rules = ruleset_for(&metas, &["java/empty-catch-block"], &Default::default());
        let a = plugin.analyze(
            &SourceFile {
                path: "src/A.java",
                text: src,
                is_test: false,
            },
            &rules,
        );
        assert_eq!(a.syntax_errors, 0);
        let f: Vec<_> = a
            .findings
            .iter()
            .map(|f| (f.range.start_line, f.range.start_col))
            .collect();
        assert_eq!(f, vec![(10, 18)]);
    }
}
