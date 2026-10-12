//! Helpers shared by the batch-4 rules: cached field ids, bounded subtree visits, Java string
//! literal decoding, annotation and type-name matching. Nothing here walks the whole tree.

use crate::prelude::*;
use std::sync::LazyLock;

/// Grammar fields used by this batch, resolved once (`child_by_field_name` looks the name up
/// on every call; these rules run on very common nodes such as every method call).
#[derive(Clone, Copy)]
pub(super) enum F {
    Name,
    Object,
    Arguments,
    Value,
    Type,
    Left,
    Right,
    Operator,
    Body,
    Condition,
    Consequence,
    Alternative,
    Field,
    Operand,
    Parameters,
    Array,
    Index,
}

const FIELD_NAMES: [&str; 17] = [
    "name",
    "object",
    "arguments",
    "value",
    "type",
    "left",
    "right",
    "operator",
    "body",
    "condition",
    "consequence",
    "alternative",
    "field",
    "operand",
    "parameters",
    "array",
    "index",
];

static FIELD_IDS: LazyLock<[u16; 17]> = LazyLock::new(|| {
    let lang = tree_sitter::Language::new(tree_sitter_java::LANGUAGE);
    FIELD_NAMES.map(|n| lang.field_id_for_name(n).map(|f| f.get()).unwrap_or(0))
});

/// Field access by cached id.
#[inline]
pub(super) fn fld(node: Node<'_>, f: F) -> Option<Node<'_>> {
    let id = FIELD_IDS.get(f as usize).copied().unwrap_or(0);
    if id == 0 {
        return None;
    }
    node.child_by_field_id(id)
}

/// The simple method name of a `method_invocation` (or the name of any declaration), or `""`.
#[inline]
pub(super) fn name_text<'t>(ctx: &Ctx<'t>, node: Node<'t>) -> &'t str {
    fld(node, F::Name).map(|n| ctx.text(n)).unwrap_or("")
}

/// The argument expressions of a call or object creation (comments skipped).
pub(super) fn args(call: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    fld(call, F::Arguments).into_iter().flat_map(named_children)
}

/// Lists this short are scanned by index (no tree-cursor allocation); longer ones with a cursor.
const SHORT: usize = 8;

/// Number of arguments of a call (comments excluded), without allocating for short lists.
pub(super) fn arg_count(call: Node<'_>) -> usize {
    let Some(a) = fld(call, F::Arguments) else {
        return 0;
    };
    let n = a.named_child_count();
    if n <= SHORT {
        (0..n)
            .filter(|i| {
                a.named_child(*i as u32)
                    .is_some_and(|c| !syntax::is_comment(c))
            })
            .count()
    } else {
        named_children(a).count()
    }
}

/// The first named, non-comment child, without allocating a tree cursor in the common case.
pub(super) fn first_child(node: Node<'_>) -> Option<Node<'_>> {
    let n = node.named_child_count();
    for i in 0..n.min(SHORT) {
        let c = node.named_child(i as u32)?;
        if !syntax::is_comment(c) {
            return Some(c);
        }
    }
    if n > SHORT {
        syntax::first_named_child(node)
    } else {
        None
    }
}

/// The first argument of a call.
pub(super) fn first_arg(call: Node<'_>) -> Option<Node<'_>> {
    fld(call, F::Arguments).and_then(first_child)
}

/// The operator of an expression as a string (`"=="`, `"+="`), or `""`.
#[inline]
pub(super) fn op_str(expr: Node<'_>) -> &'static str {
    fld(expr, F::Operator).map(kind).unwrap_or("")
}

/// What to do after visiting a node in [`visit`].
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Walk {
    /// Descend into the node's children.
    Continue,
    /// Don't look inside this node.
    Skip,
    /// End the whole visit.
    Stop,
}

/// Pre-order visit of the **named** descendants of `root` (not `root` itself), in source order.
/// Returns `true` when the callback stopped the visit. Linear in the size of the subtree.
pub(super) fn visit<'t>(root: Node<'t>, mut f: impl FnMut(Node<'t>) -> Walk) -> bool {
    let mut cursor = root.walk();
    if !cursor.goto_first_child() {
        return false;
    }
    loop {
        let n = cursor.node();
        let action = if n.is_named() { f(n) } else { Walk::Skip };
        match action {
            Walk::Stop => return true,
            Walk::Continue if cursor.goto_first_child() => continue,
            _ => {}
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() || cursor.node().id() == root.id() {
                return false;
            }
        }
    }
}

/// Whether any named node of the subtree (the root included) satisfies `pred`; subtrees for
/// which `skip` returns `true` are not entered.
pub(super) fn any_in<'t>(
    root: Node<'t>,
    mut skip: impl FnMut(Node<'t>) -> bool,
    mut pred: impl FnMut(Node<'t>) -> bool,
) -> bool {
    if pred(root) {
        return true;
    }
    visit(root, |n| {
        if skip(n) {
            Walk::Skip
        } else if pred(n) {
            Walk::Stop
        } else {
            Walk::Continue
        }
    })
}

/// Whether an expression may have an effect or yield a fresh value each time it is evaluated.
pub(super) fn has_side_effects(node: Node<'_>) -> bool {
    any_in(
        node,
        |_| false,
        |n| {
            matches!(
                kind(n),
                "method_invocation"
                    | "object_creation_expression"
                    | "array_creation_expression"
                    | "assignment_expression"
                    | "update_expression"
                    | "lambda_expression"
                    | "method_reference"
                    | "switch_expression"
                    | "ERROR"
            )
        },
    )
}

/// Whether an `identifier` token with the given text occurs in the subtree.
pub(super) fn mentions<'t>(ctx: &Ctx<'t>, node: Node<'t>, name: &str) -> bool {
    if !ctx.text(node).contains(name) {
        return false;
    }
    any_in(
        node,
        |_| false,
        |n| kind(n) == "identifier" && ctx.text(n) == name,
    )
}

/// The field declaration and declarator named `name` declared directly in type body `body`.
pub(super) fn find_field<'t>(
    ctx: &Ctx<'t>,
    body: Node<'t>,
    name: &str,
) -> Option<(Node<'t>, Node<'t>)> {
    ctx.members(body)
        .filter(|m| matches!(kind(*m), "field_declaration" | "constant_declaration"))
        .find_map(|m| {
            syntax::declarators(m)
                .find(|d| syntax::name_of(*d, ctx.source()) == name)
                .map(|d| (m, d))
        })
}

/// Whether `ann` (`annotation` / `marker_annotation`) is one of `fqns`, through a single-type
/// import, an on-demand import, or a fully-qualified name.
pub(super) fn annotation_is(ctx: &Ctx<'_>, ann: Node<'_>, fqns: &[&str]) -> bool {
    let Some(name) = fld(ann, F::Name) else {
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

/// The type node without type arguments (`Foo` of `Foo<T>`).
pub(super) fn type_base(type_node: Node<'_>) -> Node<'_> {
    if kind(type_node) == "generic_type" {
        syntax::first_named_child(type_node).unwrap_or(type_node)
    } else {
        type_node
    }
}

/// Fully-qualified name of a class type node as seen from its position (imports, this file,
/// `java.lang`, imported library types). `None` when unresolvable.
pub(super) fn type_fqn<'t>(ctx: &Ctx<'t>, type_node: Node<'t>) -> Option<String> {
    let base = type_base(type_node);
    if !matches!(kind(base), "type_identifier" | "scoped_type_identifier") {
        return None;
    }
    let text = ctx.text(base);
    if text.contains(|c: char| c.is_whitespace() || c == '/') {
        return None;
    }
    ctx.resolve_type_name(text, base)
}

/// Whether `expr` names the type `fqn` (the receiver of a static call like `Pattern.compile`).
pub(super) fn names_type<'t>(ctx: &Ctx<'t>, expr: Node<'t>, fqn: &str) -> bool {
    let simple = fqn.rsplit('.').next().unwrap_or(fqn);
    let text = ctx.text(expr);
    if text != simple && text != fqn {
        return false;
    }
    ctx.type_name_of(expr).as_deref() == Some(fqn)
}

/// A plain (non-text-block) string literal decoded to its runtime value. `None` for text
/// blocks, Unicode escapes, and anything unusual.
pub(super) fn string_value(ctx: &Ctx<'_>, node: Node<'_>) -> Option<String> {
    let node = syntax::strip_parens(node);
    if kind(node) != "string_literal" {
        return None;
    }
    let text = ctx.text(node);
    if text.starts_with("\"\"\"") {
        return None;
    }
    let raw = syntax::string_literal_content(node, ctx.source())?;
    unescape(raw)
}

/// Like [`string_value`] but text blocks are accepted too (their raw content, escapes decoded,
/// incidental indentation kept): good enough for counting placeholders.
pub(super) fn string_value_loose(ctx: &Ctx<'_>, node: Node<'_>) -> Option<String> {
    let node = syntax::strip_parens(node);
    let raw = syntax::string_literal_content(node, ctx.source())?;
    unescape(raw)
}

/// Decode the escape sequences of Java string-literal source text. `None` on Unicode escapes
/// (they are processed before tokenization, so the raw text may not mean what it looks like)
/// or unknown escapes.
pub(super) fn unescape(raw: &str) -> Option<String> {
    if !raw.contains('\\') {
        return Some(raw.to_string());
    }
    if raw.contains("\\u") {
        return None;
    }
    let mut out = String::with_capacity(raw.len());
    let mut it = raw.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let e = it.next()?;
        match e {
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'r' => out.push('\r'),
            'b' => out.push('\u{8}'),
            'f' => out.push('\u{c}'),
            's' => out.push(' '),
            '\\' => out.push('\\'),
            '"' => out.push('"'),
            '\'' => out.push('\''),
            '\n' => {}
            '\r' => {
                if it.peek() == Some(&'\n') {
                    it.next();
                }
            }
            '0'..='7' => {
                let mut v = e.to_digit(8)?;
                let max_digits = if e <= '3' { 3 } else { 2 };
                for _ in 1..max_digits {
                    match it.peek().and_then(|d| d.to_digit(8)) {
                        Some(d) => {
                            v = v * 8 + d;
                            it.next();
                        }
                        None => break,
                    }
                }
                out.push(char::from_u32(v)?);
            }
            _ => return None,
        }
    }
    Some(out)
}

// The syntactic index of declared types these rules used to keep privately is now shared:
// `ctx.declared_types()` (filled by the walker, complete in `finish`).

/// The simple name of a type node without type arguments or package: `Map` for
/// `java.util.Map<K, V>`, `int` for `int`, `String` for `String[]`.
pub(super) fn simple_type_name<'t>(ctx: &Ctx<'t>, ty: Node<'t>) -> &'t str {
    let mut t = ty;
    if kind(t) == "array_type" {
        if let Some(e) = t.field(Field::Element) {
            t = e;
        }
    }
    let text = ctx.text(type_base(t));
    text.rsplit('.').next().unwrap_or(text).trim()
}

/// The variable or field a receiver or target names: `x`, or `f` of `this.f`.
pub(super) fn var_name<'t>(ctx: &Ctx<'t>, expr: Node<'t>) -> Option<&'t str> {
    let expr = syntax::strip_parens(expr);
    match kind(expr) {
        "identifier" => Some(ctx.text(expr)),
        "field_access" => match (fld(expr, F::Object), fld(expr, F::Field)) {
            (Some(o), Some(f)) if kind(o) == "this" => Some(ctx.text(f)),
            _ => None,
        },
        _ => None,
    }
}

/// Per-file "does the source text contain `needle`?", computed on first use. Lets a rule skip
/// resolver work in the (many) files that cannot contain what it looks for.
#[derive(Default)]
pub(super) struct Gate(Option<bool>);

impl Gate {
    pub(super) fn open(&mut self, ctx: &Ctx<'_>, needle: &str) -> bool {
        *self.0.get_or_insert_with(|| ctx.source().contains(needle))
    }

    /// Like [`Gate::open`], true when any of the needles occurs.
    pub(super) fn open_any(&mut self, ctx: &Ctx<'_>, needles: &[&str]) -> bool {
        *self
            .0
            .get_or_insert_with(|| needles.iter().any(|n| ctx.source().contains(n)))
    }
}

/// Whether `node` lies inside a `synchronized` block or method, without crossing a lambda or a
/// class body.
pub(super) fn in_synchronized<'t>(ctx: &Ctx<'t>, node: Node<'t>) -> bool {
    for a in ctx.ancestors(node) {
        match kind(a) {
            "synchronized_statement" => return true,
            "method_declaration" => return syntax::has_modifier(a, "synchronized"),
            "lambda_expression"
            | "constructor_declaration"
            | "compact_constructor_declaration"
            | "static_initializer"
            | "class_body"
            | "interface_body"
            | "enum_body"
            | "enum_body_declarations" => return false,
            _ => {}
        }
    }
    false
}

/// Leading whitespace of the line that contains byte offset `at`, and whether `at` is the first
/// non-blank character of that line.
pub(super) fn line_indent(src: &str, at: usize) -> (&str, bool) {
    let at = at.min(src.len());
    let start = src
        .get(..at)
        .and_then(|s| s.rfind('\n'))
        .map_or(0, |p| p + 1);
    let line = src.get(start..).unwrap_or("");
    let ws_len = line
        .bytes()
        .take_while(|b| *b == b' ' || *b == b'\t')
        .count();
    let indent = line.get(..ws_len).unwrap_or("");
    (indent, start + ws_len == at)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unescapes_java_strings() {
        assert_eq!(unescape(r"a\\d").as_deref(), Some(r"a\d"));
        assert_eq!(unescape(r#"\"x\"\n"#).as_deref(), Some("\"x\"\n"));
        assert_eq!(unescape(r"\101\0").as_deref(), Some("A\0"));
        let unicode_escape: String = ['\\', 'u', '0', '0', '4', '1'].iter().collect();
        assert_eq!(unescape(&unicode_escape), None);
        assert_eq!(unescape(r"\q"), None);
        assert_eq!(unescape(r"end\"), None);
    }

    #[test]
    fn line_indentation() {
        let src = "a\n    b c\n\tx";
        assert_eq!(line_indent(src, 6), ("    ", true));
        assert_eq!(line_indent(src, 8), ("    ", false));
        assert_eq!(line_indent(src, 11), ("\t", true));
    }
}
