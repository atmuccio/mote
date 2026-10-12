//! Helpers shared by the rules of batch 15.

use crate::prelude::*;
use tree_sitter::TreeCursor;

/// Whether the text of a `block_comment` is a documentation comment as the documentation tool
/// reads it: it starts with `/**`, is not the empty comment `/**/`, and is not a banner that
/// starts with three or more stars (`/*****`), which is decoration rather than documentation.
pub(super) fn is_doc_comment(text: &str) -> bool {
    text.starts_with("/**") && text != "/**/" && !text.starts_with("/***")
}

/// The anonymous keyword child `kw` of `node` (`else`, `while`, `static`, …).
pub(super) fn keyword<'t>(node: Node<'t>, kw: &str) -> Option<Node<'t>> {
    syntax::children(node).find(|c| !c.is_named() && kind(*c) == kw)
}

/// Whether `ann` (`annotation` / `marker_annotation`) is one of `fqns`, written in full or
/// brought in by a single-type import or an on-demand import of its package.
pub(super) fn annotation_is(ctx: &Ctx<'_>, ann: Node<'_>, fqns: &[&str]) -> bool {
    let Some(name) = ann.field(Field::Name) else {
        return false;
    };
    let text = ctx.text(name);
    if text.contains('.') {
        return fqns.contains(&text);
    }
    if let Some(imp) = ctx
        .imports()
        .iter()
        .find(|i| !i.is_static && !i.is_wildcard && i.simple_name() == text)
    {
        return fqns.contains(&imp.path);
    }
    ctx.imports()
        .iter()
        .filter(|i| !i.is_static && i.is_wildcard)
        .any(|imp| {
            fqns.iter()
                .any(|f| f.strip_suffix(text).and_then(|q| q.strip_suffix('.')) == Some(imp.path))
        })
}

/// The file name without directories and without `.java`, when it is a Java identifier (so it
/// can name a type: not `package-info`, `module-info` or a template such as `x.header.java`).
pub(super) fn type_like_file_stem(path: &str) -> Option<&str> {
    let file = path.rsplit('/').next().unwrap_or(path);
    let stem = file.strip_suffix(".java")?;
    let mut chars = stem.chars();
    let first = chars.next()?;
    let ok = (first.is_alphabetic() || first == '_' || first == '$')
        && chars.all(|c| c.is_alphanumeric() || c == '_' || c == '$');
    ok.then_some(stem)
}

/// A cursor on `node`, a child of `parent`, reached by byte offset rather than by walking every
/// earlier sibling (long class bodies and blocks hold hundreds of children). `None` if the
/// cursor cannot reach `node`.
pub(super) fn cursor_at<'t>(parent: Node<'t>, node: Node<'t>) -> Option<TreeCursor<'t>> {
    let mut c = parent.walk();
    c.goto_first_child_for_byte(node.start_byte())?;
    // Zero-width or touching siblings may come first.
    while c.node().id() != node.id() {
        if c.node().start_byte() > node.start_byte() || !c.goto_next_sibling() {
            return None;
        }
    }
    Some(c)
}

/// The first named child of `parent` after `node` that is not a comment.
pub(super) fn next_code<'t>(parent: Node<'t>, node: Node<'t>) -> Option<Node<'t>> {
    let mut c = cursor_at(parent, node)?;
    while c.goto_next_sibling() {
        let n = c.node();
        if n.is_named() && !syntax::is_comment(n) {
            return Some(n);
        }
    }
    None
}

/// The top-level type declarations of a file, read backwards from its end (types follow the
/// package line and the imports, so this rarely visits more than the types). `None` for a
/// compact source file (top-level methods or fields, Java 25), which declares an implicit class.
pub(super) fn top_level_types(program: Node<'_>) -> Option<Vec<Node<'_>>> {
    let mut types = Vec::new();
    let mut c = program.walk();
    if c.goto_last_child() {
        loop {
            let n = c.node();
            if n.is_named() {
                if syntax::is_type_declaration(n) {
                    types.push(n);
                } else if matches!(kind(n), "package_declaration" | "import_declaration") {
                    break;
                } else if matches!(kind(n), "method_declaration" | "field_declaration") {
                    return None;
                }
            }
            if !c.goto_previous_sibling() {
                break;
            }
        }
    }
    types.reverse();
    Some(types)
}

/// Whether `text` contains `name` as a whole identifier (not as part of a longer one).
#[cfg(test)]
pub(super) fn mentions_identifier(text: &str, name: &str) -> bool {
    find_identifier(text, name).is_some()
}

/// The byte offset of the first occurrence of `name` in `text` as a whole identifier (not as
/// part of a longer one).
pub(super) fn find_identifier(text: &str, name: &str) -> Option<usize> {
    if name.is_empty() {
        return None;
    }
    let bytes = text.as_bytes();
    let is_ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'$' || b >= 0x80;
    let mut from = 0;
    while let Some(i) = text[from..].find(name) {
        let start = from + i;
        let end = start + name.len();
        let before_ok = start == 0 || !is_ident(bytes[start - 1]);
        let after_ok = end >= bytes.len() || !is_ident(bytes[end]);
        if before_ok && after_ok {
            return Some(start);
        }
        from = end;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doc_comments() {
        assert!(is_doc_comment("/** Doc. */"));
        assert!(is_doc_comment("/**\n * Doc.\n */"));
        assert!(!is_doc_comment("/**/"));
        assert!(!is_doc_comment("/*********** banner ***/"));
        assert!(!is_doc_comment("/* plain */"));
    }

    #[test]
    fn file_stems() {
        assert_eq!(type_like_file_stem("src/a/Foo.java"), Some("Foo"));
        assert_eq!(type_like_file_stem("Foo.java"), Some("Foo"));
        assert_eq!(type_like_file_stem("src/package-info.java"), None);
        assert_eq!(type_like_file_stem("src/x.header.java"), None);
        assert_eq!(type_like_file_stem("src/Foo.txt"), None);
    }

    #[test]
    fn identifiers() {
        assert!(mentions_identifier("x = 1;", "x"));
        assert!(mentions_identifier("foo(a, x)", "x"));
        assert!(!mentions_identifier("xs = 1; max = 2;", "x"));
        assert!(!mentions_identifier("x_1 + $x", "x"));
        assert!(mentions_identifier("a.x", "x"));
    }
}
