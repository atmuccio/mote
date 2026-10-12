//! Pre-parse workaround for a grammar gap: tree-sitter-java 0.23 rejects a type annotation on
//! the ellipsis of a variable-arity parameter (`String @Nullable ... args`, valid since Java 8)
//! and recovers with errors that degrade the whole tree.
//!
//! Before parsing, such annotations are replaced by spaces (newlines kept), so the grammar sees
//! `String            ... args`. Every byte keeps its offset, line and column: nodes, findings
//! and the text rules read (always taken from the original source) are unchanged. Only
//! annotations that sit between a type and `...` are touched; a file without `...` is not even
//! scanned.

use std::borrow::Cow;

fn is_ident_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$' || c >= 0x80
}

fn skip_space_back(b: &[u8], mut i: usize) -> usize {
    while i > 0 && b[i - 1].is_ascii_whitespace() {
        i -= 1;
    }
    i
}

/// The start of an annotation that ends right before `end` (`@Name`, `@a.b.Name`,
/// `@Name(args)`), or `None`.
fn annotation_start(b: &[u8], end: usize) -> Option<usize> {
    let mut i = end;
    if i > 0 && b[i - 1] == b')' {
        // Arguments: find the matching `(`; give up on string or character literals.
        let mut depth = 0usize;
        loop {
            if i == 0 {
                return None;
            }
            i -= 1;
            match b[i] {
                b')' => depth += 1,
                b'(' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                b'"' | b'\'' | b';' | b'{' | b'}' => return None,
                _ => {}
            }
        }
        i = skip_space_back(b, i);
    }
    // The (qualified) name.
    let name_end = i;
    loop {
        while i > 0 && is_ident_byte(b[i - 1]) {
            i -= 1;
        }
        if i > 0 && b[i - 1] == b'.' && i - 1 > 0 && is_ident_byte(b[i - 2]) {
            i -= 1;
            continue;
        }
        break;
    }
    if i == name_end || i == 0 || b[i - 1] != b'@' {
        return None;
    }
    // Not an annotation type declaration (`@interface`) nor an ident starting with a digit.
    if b[i].is_ascii_digit() || &b[i..name_end] == b"interface" {
        return None;
    }
    Some(i - 1)
}

/// `text` with every annotation between a parameter type and `...` blanked out, or `text`
/// itself when there is none.
pub(crate) fn blank_varargs_annotations(text: &str) -> Cow<'_, str> {
    if !text.contains("...") || !text.contains('@') {
        return Cow::Borrowed(text);
    }
    let b = text.as_bytes();
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut from = 0;
    while let Some(pos) = text[from..].find("...") {
        let dots = from + pos;
        from = dots + 3;
        // A parameter name must follow: `... args`.
        let after = text[from..].trim_start();
        if !after
            .bytes()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == b'_' || c == b'$')
        {
            continue;
        }
        // One or more annotations right before the dots, then the end of a type.
        let mut end = skip_space_back(b, dots);
        let mut found: Vec<(usize, usize)> = Vec::new();
        while let Some(start) = annotation_start(b, end) {
            found.push((start, end));
            end = skip_space_back(b, start);
        }
        let ends_type = end > 0 && (is_ident_byte(b[end - 1]) || matches!(b[end - 1], b'>' | b']'));
        if found.is_empty() || !ends_type {
            continue;
        }
        spans.extend(found);
    }
    if spans.is_empty() {
        return Cow::Borrowed(text);
    }
    let mut out = b.to_vec();
    for (start, end) in spans {
        for c in &mut out[start..end] {
            if *c != b'\n' && *c != b'\r' {
                *c = b' ';
            }
        }
    }
    // Only ASCII bytes of annotations (and whatever multi-byte characters their names held,
    // replaced whole) were changed.
    match String::from_utf8(out) {
        Ok(s) => Cow::Owned(s),
        Err(_) => Cow::Borrowed(text),
    }
}

#[cfg(test)]
mod tests {
    use super::blank_varargs_annotations as blank;

    #[test]
    fn blanks_only_annotations_before_the_ellipsis() {
        let src = "void f(String @Nullable ... args, Object @NonNull... xs) {}";
        let out = blank(src);
        assert_eq!(out.len(), src.len());
        assert_eq!(
            out,
            "void f(String           ... args, Object         ... xs) {}"
        );
        let src = "void g(@Deprecated String @a.b.C(\"x\") @D(1)... v) {}";
        // A string argument is not followed: the annotation with it stays, and so does `@D(1)`
        // (the chain is only removed when every link is understood).
        assert_eq!(blank(src), src);
        let src = "void h(String @A(x = {1}) ... v) {}";
        assert_eq!(blank(src), src, "braces are not followed");
        let src = "void k(String @A(1)\n  ... v) {}";
        assert_eq!(blank(src), "void k(String      \n  ... v) {}");
    }

    #[test]
    fn leaves_other_code_alone() {
        for src in [
            "void f(String... args) {}",
            "void f(@Nullable String... args) {}",
            "// see Foo#bar(String...) @since 1.0\nclass A {}",
            "class A { String s = \"@x ... y\"; }",
            "@interface A { }",
            "class A { int x = 1; }",
        ] {
            assert!(matches!(blank(src), std::borrow::Cow::Borrowed(_)), "{src}");
        }
    }

    #[test]
    fn annotated_varargs_parse_without_errors_and_keep_positions() {
        use crate::parse::{parse, ParseBudget};
        use mote_core::testing::ruleset_for;
        use mote_core::{LanguagePlugin, SourceFile};
        let src = "class A {
  static void f(String @Nullable ... a, int b) {
    try { g(); } catch (RuntimeException e) { }
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
        assert_eq!(f, vec![(3, 18)]);
    }
}
