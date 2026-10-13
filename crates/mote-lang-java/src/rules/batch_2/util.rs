//! Helpers shared by the batch-2 rules: naming presets (the workspace has no regex engine, so
//! naming rules accept a fixed set of presets) and small syntax utilities.

use crate::prelude::*;
use std::num::NonZeroU16;
use std::sync::LazyLock;

/// Grammar field ids of the fields read on hot paths. `Node::child_by_field_name` resolves the
/// name by scanning the grammar's field table on every call; rules that run on every
/// `method_invocation` or `if_statement` use these ids with [`field`] instead.
pub(crate) struct FieldIds {
    pub(crate) name: u16,
    pub(crate) object: u16,
    pub(crate) field: u16,
    pub(crate) left: u16,
    pub(crate) consequence: u16,
    pub(crate) alternative: u16,
}

pub(crate) static FIELDS: LazyLock<FieldIds> = LazyLock::new(|| {
    let lang = tree_sitter::Language::new(tree_sitter_java_orchard::LANGUAGE);
    let id = |n: &str| lang.field_id_for_name(n).map_or(0, NonZeroU16::get);
    FieldIds {
        name: id("name"),
        object: id("object"),
        field: id("field"),
        left: id("left"),
        consequence: id("consequence"),
        alternative: id("alternative"),
    }
});

/// The child in field `id` (from [`FIELDS`]); `None` for an unknown id.
#[inline]
pub(crate) fn field(node: Node<'_>, id: u16) -> Option<Node<'_>> {
    if id == 0 {
        None
    } else {
        node.child_by_field_id(id)
    }
}

/// A naming convention preset accepted by the `format` parameter of the naming rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NameFormat {
    /// `^[A-Z][a-zA-Z0-9]*$`
    Pascal,
    /// `^[a-z][a-zA-Z0-9]*$`
    Camel,
    /// `^[A-Z][A-Z0-9]*(_[A-Z0-9]+)*$`
    UpperSnake,
    /// `^[a-z][a-z0-9]*(_[a-z0-9]+)*$`
    Snake,
    /// `^[a-z_][a-z0-9_]*(\.[a-z_][a-z0-9_]*)*$`
    LowerDotted,
}

impl NameFormat {
    /// Parse a preset name (or the exact regular expression a preset stands for). Unknown
    /// values yield `None`: the rule then stays silent rather than guessing.
    pub(crate) fn parse(value: &str) -> Option<NameFormat> {
        let v = value.trim();
        let f = match v {
            "PascalCase" | "pascal" | "^[A-Z][a-zA-Z0-9]*$" => NameFormat::Pascal,
            "camelCase" | "camel" | "^[a-z][a-zA-Z0-9]*$" => NameFormat::Camel,
            "UPPER_SNAKE" | "upper_snake" | "^[A-Z][A-Z0-9]*(_[A-Z0-9]+)*$" => {
                NameFormat::UpperSnake
            }
            "snake_case" | "snake" | "^[a-z][a-z0-9]*(_[a-z0-9]+)*$" => NameFormat::Snake,
            "lower.dotted" | "lower_dotted" | r"^[a-z_][a-z0-9_]*(\.[a-z_][a-z0-9_]*)*$" => {
                NameFormat::LowerDotted
            }
            _ => return None,
        };
        Some(f)
    }

    /// Human-readable name used in messages.
    pub(crate) fn label(self) -> &'static str {
        match self {
            NameFormat::Pascal => "PascalCase",
            NameFormat::Camel => "camelCase",
            NameFormat::UpperSnake => "UPPER_SNAKE",
            NameFormat::Snake => "snake_case",
            NameFormat::LowerDotted => "lower.dotted",
        }
    }

    /// Whether `name` follows the convention. Letters are classified by Unicode case, so `Äpfel`
    /// is PascalCase; letters without case (CJK, …) are accepted wherever a letter is.
    pub(crate) fn matches(self, name: &str) -> bool {
        self.matches_with(name, false)
    }

    /// Like [`matches`](Self::matches), but underscores are accepted after the first character
    /// (test method names such as `rejects_empty_input`).
    pub(crate) fn matches_allowing_underscores(self, name: &str) -> bool {
        self.matches_with(name, true)
    }

    fn matches_with(self, name: &str, underscores: bool) -> bool {
        match self {
            NameFormat::Pascal => alnum_tail(name, upper_like, underscores),
            NameFormat::Camel => alnum_tail(name, lower_like, underscores),
            NameFormat::UpperSnake => snake(name, upper_like),
            NameFormat::Snake => snake(name, lower_like),
            NameFormat::LowerDotted => {
                !name.is_empty()
                    && name.split('.').all(|seg| {
                        let mut chars = seg.chars();
                        chars.next().is_some_and(|c| c == '_' || lower_like(c))
                            && chars.all(|c| c == '_' || c.is_numeric() || lower_like(c))
                    })
            }
        }
    }
}

/// An uppercase letter, or a letter without case.
fn upper_like(c: char) -> bool {
    if c.is_ascii() {
        c.is_ascii_uppercase()
    } else {
        c.is_alphabetic() && !c.is_lowercase()
    }
}

/// A lowercase letter, or a letter without case.
fn lower_like(c: char) -> bool {
    if c.is_ascii() {
        c.is_ascii_lowercase()
    } else {
        c.is_alphabetic() && !c.is_uppercase()
    }
}

/// `^[first][letters digits]*$` (plus `_` after the first character when `underscores`).
/// An underscore between two digits is always accepted: digits have no case, so it is the only
/// way to separate the parts of a version number (`isBefore3_8`).
fn alnum_tail(name: &str, first: fn(char) -> bool, underscores: bool) -> bool {
    let b = name.as_bytes();
    let digit_at = |i: usize| b.get(i).is_some_and(u8::is_ascii_digit);
    let mut chars = name.char_indices();
    chars.next().is_some_and(|(_, c)| first(c))
        && chars.all(|(i, c)| {
            c.is_alphanumeric()
                || (c == '_' && (underscores || (i > 0 && digit_at(i - 1) && digit_at(i + 1))))
        })
}

/// Whether the file binds native code through JNA (or a similar library): type, method, and
/// field names there mirror native symbols (`SERVICE_STATUS`, `ShellExecuteEx`) and cannot
/// follow Java conventions.
pub(crate) fn binds_native_code(ctx: &Ctx<'_>) -> bool {
    ctx.imports().iter().any(|i| {
        let p = i.path;
        p.starts_with("com.sun.jna") || p.starts_with("jnr.ffi") || p.starts_with("org.bridj")
    })
}

/// `^[case][case digits]*(_[case digits]+)*$`
fn snake(name: &str, case: fn(char) -> bool) -> bool {
    name.chars().next().is_some_and(case)
        && name
            .split('_')
            .all(|seg| !seg.is_empty() && seg.chars().all(|c| case(c) || c.is_numeric()))
}

/// The `format` parameter of a naming rule, parsed on first use and kept for the file (rule
/// instances are per file, so the cache never outlives the configuration it was read from).
#[derive(Default)]
pub(crate) struct FormatCache(Option<Option<NameFormat>>);

impl FormatCache {
    pub(crate) fn get(&mut self, ctx: &Ctx<'_>) -> Option<NameFormat> {
        *self
            .0
            .get_or_insert_with(|| NameFormat::parse(ctx.param_str("format")))
    }
}

/// Whether `name` occurs in `src` as a whole word (not inside a longer identifier) outside the
/// byte range `[skip_start, skip_end)`. Code, strings, and comments all count, which makes the
/// unused-declaration rules conservative: a name mentioned anywhere else is considered used.
pub(crate) fn occurs_outside(src: &str, name: &str, skip_start: usize, skip_end: usize) -> bool {
    if name.is_empty() {
        return true;
    }
    let b = src.as_bytes();
    let is_ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c >= 0x80;
    src.match_indices(name).any(|(i, _)| {
        let end = i + name.len();
        (i < skip_start || i >= skip_end)
            && (i == 0 || !is_ident(b[i - 1]))
            && b.get(end).is_none_or(|c| !is_ident(*c))
    })
}

/// The `modifiers` of a declaration, read without creating tree cursors (which allocate):
/// the rules on fields, methods, and locals query them several times per declaration.
#[derive(Clone, Copy)]
pub(crate) struct Mods<'t>(Option<Node<'t>>);

impl<'t> Mods<'t> {
    pub(crate) fn of(decl: Node<'t>) -> Self {
        Mods(match decl.child(0) {
            Some(c) if kind(c) == "modifiers" => Some(c),
            // A leading comment: let the general helper find the modifiers.
            Some(c) if syntax::is_comment(c) => syntax::modifiers(decl),
            _ => None,
        })
    }

    fn children(self) -> impl Iterator<Item = Node<'t>> {
        let m = self.0;
        let n = m.map_or(0, |m| m.child_count());
        (0..n).filter_map(move |i| m.and_then(|m| m.child(i)))
    }

    /// The keyword modifier `kw` (`"static"`, `"final"`, `"private"`, …) is present.
    pub(crate) fn has(self, kw: &str) -> bool {
        self.children()
            .filter_map(syntax::modifier_keyword)
            .any(|c| kind(c) == kw)
    }

    /// Any annotation is present.
    pub(crate) fn annotated(self) -> bool {
        self.children()
            .any(|c| matches!(kind(c), "annotation" | "marker_annotation"))
    }

    /// An annotation with simple name `name` (`Override`, `java.lang.Override`) is present.
    pub(crate) fn has_annotation(self, src: &str, name: &str) -> bool {
        self.children().any(|c| {
            matches!(kind(c), "annotation" | "marker_annotation")
                && syntax::annotation_name(c, src) == name
        })
    }
}

/// The `if`/`else`/… keyword token of a statement (its first child), for reporting.
pub(crate) fn keyword<'t>(node: Node<'t>) -> Node<'t> {
    syntax::children(node).next().unwrap_or(node)
}

/// The next sibling statement of `node`, skipping comments.
pub(crate) fn next_statement<'t>(ctx: &Ctx<'t>, node: Node<'t>) -> Option<Node<'t>> {
    let mut cur = ctx.next_named_sibling(node);
    while let Some(n) = cur {
        if !syntax::is_comment(n) {
            return Some(n);
        }
        cur = ctx.next_named_sibling(n);
    }
    None
}

/// The only statement of a block (comments ignored), or the statement itself when it is not
/// a block. `None` for empty blocks or blocks with several statements.
pub(crate) fn single_statement(stmt: Node<'_>) -> Option<Node<'_>> {
    if kind(stmt) != "block" {
        return Some(stmt);
    }
    let mut it = named_children(stmt);
    let first = it.next()?;
    if it.next().is_some() {
        return None;
    }
    Some(first)
}

/// Whether a member declared at `node` can be seen from outside its file's private parts:
/// every enclosing type is a named, non-private, non-local type. Members of private nested
/// classes, local classes, and anonymous classes are effectively private.
pub(crate) fn is_accessible_member<'t>(node: Node<'t>, ctx: &Ctx<'t>) -> bool {
    let mut prev: Option<Node<'t>> = None;
    for a in ctx.ancestors(node) {
        match kind(a) {
            "object_creation_expression" | "enum_constant"
                if prev.is_some_and(|p| kind(p) == "class_body") =>
            {
                return false;
            }
            "block" | "constructor_body" | "switch_block_statement_group" | "switch_rule"
                if prev.is_some_and(syntax::is_type_declaration) =>
            {
                return false;
            }
            _ if syntax::is_type_declaration(a) && syntax::has_modifier(a, "private") => {
                return false;
            }
            _ => {}
        }
        prev = Some(a);
    }
    true
}

/// Shorten a string for a message (at most `max` characters, `…` appended when cut).
pub(crate) fn shorten(s: &str, max: usize) -> String {
    let mut out: String = s.chars().take(max).collect();
    if s.chars().nth(max).is_some() {
        out.push('…');
    }
    out.replace(['\n', '\r'], " ")
}

/// Whether `node` lies inside a JUnit 3 style test class: a class whose superclass's simple name
/// is `TestCase` or ends with it (`junit.framework.TestCase`, a project's `BaseTestCase`). Such
/// classes are test code even when a module keeps them under a main source root, where the
/// path-based test detection does not see them.
pub(crate) fn in_junit3_test_class<'t>(ctx: &Ctx<'t>, node: Node<'t>) -> bool {
    ctx.ancestors(node).any(|a| {
        kind(a) == "class_declaration"
            && a.field(Field::Superclass)
                .and_then(syntax::first_named_child)
                .is_some_and(|t| {
                    let base = if kind(t) == "generic_type" {
                        syntax::first_named_child(t).unwrap_or(t)
                    } else {
                        t
                    };
                    let text = ctx.text(base);
                    text.rsplit('.')
                        .next()
                        .is_some_and(|simple| simple.ends_with("TestCase"))
                })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_match() {
        let p = NameFormat::parse("PascalCase").unwrap();
        assert!(p.matches("HttpClient") && p.matches("A1") && !p.matches("httpClient"));
        assert!(!p.matches("Http_Client") && !p.matches(""));
        let c = NameFormat::parse("^[a-z][a-zA-Z0-9]*$").unwrap();
        assert_eq!(c, NameFormat::Camel);
        assert!(c.matches("fooBar") && !c.matches("FooBar") && !c.matches("foo_bar"));
        assert!(
            c.matches_allowing_underscores("foo_bar") && !c.matches_allowing_underscores("_foo")
        );
        let u = NameFormat::parse("UPPER_SNAKE").unwrap();
        assert!(u.matches("MAX_SIZE") && u.matches("A") && u.matches("V2_X"));
        assert!(
            !u.matches("MAX__SIZE")
                && !u.matches("_MAX")
                && !u.matches("MAX_")
                && !u.matches("Max")
        );
        let d = NameFormat::parse(r"^[a-z_][a-z0-9_]*(\.[a-z_][a-z0-9_]*)*$").unwrap();
        assert!(d.matches("org.acme.foo_bar") && d.matches("_x.y2"));
        assert!(!d.matches("org.Acme") && !d.matches("org..x") && !d.matches("org.2x"));
        assert!(NameFormat::parse("[a-z]+").is_none());
        // Unicode letters are classified by case; multi-byte first characters never panic.
        assert!(p.matches("Äpfel") && p.matches("Ä") && !p.matches("äpfel"));
        assert!(c.matches("größe") && !c.matches("Größe") && c.matches("名前"));
        assert!(c.matches_allowing_underscores("ä_b") && !c.matches_allowing_underscores("Ä_b"));
        assert!(u.matches("ÄRGER_1") && !u.matches("Ärger"));
        // Underscores between digits (version numbers) are fine; elsewhere they are not.
        assert!(c.matches("isPre3_8") && c.matches("v1_2_3") && p.matches("Api2_0"));
        assert!(!c.matches("is_3") && !c.matches("x3_") && !c.matches("a_b"));
    }

    #[test]
    fn occurrences_and_shorten() {
        let src = "class A { private void run() {} void go() { runAll(); } } // Outer$Inner";
        let decl = src.find("private").unwrap();
        let decl_end = src.find("{}").unwrap() + 2;
        assert!(!occurs_outside(src, "run", decl, decl_end)); // `runAll` is another word
        assert!(occurs_outside(src, "runAll", decl, decl_end));
        assert!(occurs_outside(src, "Inner", 0, 0)); // `$` separates nested class names
        assert!(!occurs_outside(src, "Out", 0, 0));
        assert!(occurs_outside(src, "A", 0, 0) && occurs_outside("x", "", 0, 1));
        assert_eq!(shorten("abcdef", 3), "abc…");
        assert_eq!(shorten("ab", 3), "ab");
    }
}
