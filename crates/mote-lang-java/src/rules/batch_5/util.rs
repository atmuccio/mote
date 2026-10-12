//! Helpers shared by the batch-5 rules. Nothing here walks the whole tree: every function looks
//! at one bounded subtree (a class body, a method body, an expression) or at a node's ancestors.

use crate::prelude::*;
use std::sync::LazyLock;

// ------------------------------------------------------------------------- field ids

/// Grammar field ids, resolved once. `child_by_field_name` scans the grammar's field table on
/// every call; the rules of this batch run on very common nodes (every call, every field
/// access), so they read fields by id.
pub(super) struct FieldIds {
    pub name: u16,
    pub object: u16,
    pub arguments: u16,
    pub value: u16,
    pub ty: u16,
    pub body: u16,
    pub left: u16,
    pub right: u16,
    pub field: u16,
    pub condition: u16,
    pub consequence: u16,
    pub alternative: u16,
    pub parameters: u16,
    pub init: u16,
    pub array: u16,
    pub index: u16,
    pub superclass: u16,
    pub key: u16,
    pub operand: u16,
    pub dimensions: u16,
    pub element: u16,
    pub constructor: u16,
}

pub(super) static F: LazyLock<FieldIds> = LazyLock::new(|| {
    let lang = tree_sitter::Language::new(tree_sitter_java::LANGUAGE);
    let id = |n: &str| lang.field_id_for_name(n).map_or(0, |f| f.get());
    FieldIds {
        name: id("name"),
        object: id("object"),
        arguments: id("arguments"),
        value: id("value"),
        ty: id("type"),
        body: id("body"),
        left: id("left"),
        right: id("right"),
        field: id("field"),
        condition: id("condition"),
        consequence: id("consequence"),
        alternative: id("alternative"),
        parameters: id("parameters"),
        init: id("init"),
        array: id("array"),
        index: id("index"),
        superclass: id("superclass"),
        key: id("key"),
        operand: id("operand"),
        dimensions: id("dimensions"),
        element: id("element"),
        constructor: id("constructor"),
    }
});

/// The child in field `id` (an id from [`F`]); `None` for an unknown id.
#[inline]
pub(super) fn fld(node: Node<'_>, id: u16) -> Option<Node<'_>> {
    if id == 0 {
        None
    } else {
        node.child_by_field_id(id)
    }
}

/// The argument expressions of a call or object creation (comments skipped).
pub(super) fn args(call: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    fld(call, F.arguments).into_iter().flat_map(named_children)
}

/// Number of arguments of a call.
pub(super) fn arg_count(call: Node<'_>) -> usize {
    match fld(call, F.arguments) {
        Some(a) if a.named_child_count() > 0 => named_children(a).count(),
        _ => 0,
    }
}

/// The `i`-th argument of a call.
pub(super) fn arg(call: Node<'_>, i: usize) -> Option<Node<'_>> {
    args(call).nth(i)
}

/// The name text of a method invocation (`""` when absent).
#[inline]
pub(super) fn call_name<'t>(ctx: &Ctx<'t>, call: Node<'t>) -> &'t str {
    fld(call, F.name).map_or("", |n| ctx.text(n))
}

// ------------------------------------------------------------------------- bounded visits

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

/// Pre-order visit of the **named** descendants of `root` (not `root` itself), in source
/// order. Returns `true` when the callback stopped the visit. Linear in the subtree size.
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

/// Nodes whose code runs in another context: lambdas and class bodies (anonymous and local
/// classes).
pub(super) fn is_other_context(n: Node<'_>) -> bool {
    matches!(kind(n), "lambda_expression" | "class_body")
}

/// Tokens (leaves) of a subtree in source order, comments skipped. Iterative.
pub(super) fn for_each_token<'t>(node: Node<'t>, mut f: impl FnMut(Node<'t>) -> bool) -> bool {
    let mut c = node.walk();
    let mut depth = 0u32;
    loop {
        let n = c.node();
        let descend = !syntax::is_comment(n) && c.goto_first_child();
        if descend {
            depth += 1;
            continue;
        }
        if !syntax::is_comment(n) && !f(n) {
            return false;
        }
        loop {
            if depth == 0 {
                return true;
            }
            if c.goto_next_sibling() {
                break;
            }
            if !c.goto_parent() {
                return true;
            }
            depth -= 1;
        }
    }
}

/// Whether two subtrees are the same code, token by token (whitespace and comments ignored).
/// An identifier token equal to `rename.0` in `a` matches one equal to `rename.1` in `b`
/// (used to compare catch blocks whose exception variables have different names).
pub(super) fn same_tokens(a: Node<'_>, b: Node<'_>, src: &str, rename: (&str, &str)) -> bool {
    let mut left: Vec<&str> = Vec::new();
    for_each_token(a, |t| {
        left.push(syntax::node_text(t, src));
        left.len() < 100_000
    });
    let mut i = 0usize;
    let ok = for_each_token(b, |t| {
        let tb = syntax::node_text(t, src);
        let Some(ta) = left.get(i) else {
            return false;
        };
        i += 1;
        *ta == tb || (!rename.0.is_empty() && *ta == rename.0 && tb == rename.1)
    });
    ok && i == left.len()
}

// ------------------------------------------------------------------------- text helpers

fn is_ident_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$' || c >= 0x80
}

/// Byte offsets of the whole-word occurrences of `name` in `src[start..end]` (code, strings,
/// and comments alike).
pub(super) fn word_occurrences<'s>(
    src: &'s str,
    name: &'s str,
    start: usize,
    end: usize,
) -> impl Iterator<Item = usize> + 's {
    let b = src.as_bytes();
    let hay = src.get(start..end.min(src.len())).unwrap_or("");
    hay.match_indices(name).filter_map(move |(i, _)| {
        if name.is_empty() {
            return None;
        }
        let at = start + i;
        let after = at + name.len();
        let before_ok = at == 0 || !is_ident_byte(b[at - 1]);
        let after_ok = b.get(after).is_none_or(|c| !is_ident_byte(*c));
        (before_ok && after_ok).then_some(at)
    })
}

/// Ranges up to this many bytes are scanned by [`source_words`] without the word index.
const SCAN_DIRECTLY: usize = 4096;

/// [`word_occurrences`] in the file's source, read from the file's word index
/// ([`Ctx::word_offsets`]): looking up many names stays linear in the file, where scanning the
/// text per name is quadratic (a class with thousands of fields).
pub(super) fn source_words<'a>(
    ctx: &'a Ctx<'_>,
    name: &'a str,
    start: usize,
    end: usize,
) -> Box<dyn Iterator<Item = usize> + 'a> {
    let src = ctx.source();
    // A short range (a method body, an initializer) is scanned directly while the file has no
    // index yet: building one reads every word of the file, which many files never need. Both
    // find the same occurrences (see the test below).
    if end.min(src.len()).saturating_sub(start) <= SCAN_DIRECTLY && !ctx.has_word_index() {
        return Box::new(word_occurrences(src, name, start, end));
    }
    match ctx.word_offsets(name) {
        Some(all) => {
            let end = end.min(src.len());
            let from = all.partition_point(|&o| (o as usize) < start);
            let len = name.len();
            Box::new(
                all[from..]
                    .iter()
                    .map(|&o| o as usize)
                    .take_while(move |&o| o + len <= end),
            )
        }
        None => Box::new(word_occurrences(src, name, start, end)),
    }
}

/// [`occurs_in`] the file's source, from its word index (see [`source_words`]).
pub(super) fn occurs_in_source(ctx: &Ctx<'_>, name: &str, start: usize, end: usize) -> bool {
    source_words(ctx, name, start, end).next().is_some()
}

#[cfg(test)]
mod word_tests {
    use super::*;

    /// The word index finds exactly what the text scan finds.
    #[test]
    fn source_words_agree_with_the_text_scan() {
        let src = "class A { int x; int xx = x + ax + x$ + $x + x1; String s = \"x\"; // x\n \
                   void x() { é x; xé; x } }";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_java::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(src, None).unwrap();
        let file = mote_core::SourceFile {
            path: "A.java",
            text: src,
            is_test: false,
        };
        let index = mote_core::RuleIndex::default();
        let ctx = Ctx::new(&file, tree.root_node(), &index, &[]);
        // Build the index first: `source_words` scans short ranges directly until it exists.
        assert!(ctx.word_offsets("x").is_some() && ctx.has_word_index());
        for name in ["x", "xx", "A", "é", "xé", "int", "nope", "x$", ""] {
            for (start, end) in [(0, src.len()), (10, 40), (30, 31), (0, 0), (50, 10_000)] {
                let scan: Vec<usize> = word_occurrences(src, name, start, end).collect();
                let indexed: Vec<usize> = source_words(&ctx, name, start, end).collect();
                assert_eq!(scan, indexed, "{name:?} in {start}..{end}");
            }
        }
    }
}

/// `UPPER_SNAKE` (at least one letter, no lowercase letter): the conventional shape of a
/// constant name.
pub(super) fn is_constant_name(name: &str) -> bool {
    name.bytes().any(|c| c.is_ascii_uppercase())
        && name
            .bytes()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
}

// ------------------------------------------------------------------------- modifiers

/// The `modifiers` of a declaration, read without creating tree cursors.
#[derive(Clone, Copy)]
pub(super) struct Mods<'t>(Option<Node<'t>>);

impl<'t> Mods<'t> {
    pub(super) fn of(decl: Node<'t>) -> Self {
        Mods(match decl.child(0) {
            Some(c) if kind(c) == "modifiers" => Some(c),
            Some(c) if syntax::is_comment(c) => syntax::modifiers(decl),
            _ => None,
        })
    }

    fn children(self) -> impl Iterator<Item = Node<'t>> {
        let m = self.0;
        let n = m.map_or(0, |m| m.child_count());
        (0..n).filter_map(move |i| m.and_then(|m| m.child(i)))
    }

    /// The keyword modifier `kw` is present.
    pub(super) fn has(self, kw: &str) -> bool {
        self.children().any(|c| !c.is_named() && kind(c) == kw)
    }

    /// The keyword token `kw`, if present (for reporting).
    pub(super) fn keyword(self, kw: &str) -> Option<Node<'t>> {
        self.children().find(|c| !c.is_named() && kind(*c) == kw)
    }

    /// Any annotation is present.
    pub(super) fn annotated(self) -> bool {
        self.children()
            .any(|c| matches!(kind(c), "annotation" | "marker_annotation"))
    }

    /// An annotation with simple name `name` is present.
    pub(super) fn has_annotation(self, src: &str, name: &str) -> bool {
        self.children().any(|c| {
            matches!(kind(c), "annotation" | "marker_annotation")
                && syntax::annotation_name(c, src) == name
        })
    }

    /// The annotations.
    pub(super) fn annotations(self) -> impl Iterator<Item = Node<'t>> {
        self.children()
            .filter(|c| matches!(kind(*c), "annotation" | "marker_annotation"))
    }
}

// ------------------------------------------------------------------------- statements

/// The last named, non-comment child.
pub(super) fn last_named(node: Node<'_>) -> Option<Node<'_>> {
    named_children(node).last()
}

/// The parent of `node` and the child of that parent on the path, skipping parentheses.
pub(super) fn parent_skipping_parens<'t>(
    ctx: &Ctx<'t>,
    node: Node<'t>,
) -> Option<(Node<'t>, Node<'t>)> {
    let mut child = node;
    for a in ctx.ancestors(node) {
        if kind(a) != "parenthesized_expression" {
            return Some((a, child));
        }
        child = a;
    }
    None
}

// ------------------------------------------------------------------------- types & imports

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

/// Whether `ann` (`annotation` / `marker_annotation`) is one of `fqns`, through a single-type
/// import, an on-demand import, or a fully-qualified name.
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

/// The JUnit (or TestNG) flavour a file uses, from its imports.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(super) struct TestImports {
    /// `org.junit.jupiter…`
    pub jupiter: bool,
    /// `org.junit.*` outside `jupiter` / `platform` (JUnit 4), or `junit.framework` (JUnit 3).
    pub junit4: bool,
    /// `org.testng…`
    pub testng: bool,
}

pub(super) fn test_imports(ctx: &Ctx<'_>) -> TestImports {
    let mut t = TestImports::default();
    for i in ctx.imports() {
        let p = i.path;
        if p.starts_with("org.junit.jupiter") {
            t.jupiter = true;
        } else if p.starts_with("org.testng") {
            t.testng = true;
        } else if (p == "org.junit" || p.starts_with("org.junit.") || p.starts_with("junit."))
            && !p.starts_with("org.junit.platform")
        {
            t.junit4 = true;
        }
    }
    t
}

/// Whether the class body declares a method named `name`.
pub(super) fn declares_method<'t>(ctx: &Ctx<'t>, body: Node<'t>, name: &str) -> bool {
    ctx.members(body)
        .any(|m| kind(m) == "method_declaration" && syntax::name_of(m, ctx.source()) == name)
}

// ------------------------------------------------------------------------- literals & constants

/// A literal of any kind (numbers, characters, strings, text blocks, booleans, `null`),
/// possibly negated or parenthesized.
pub(super) fn is_literal(node: Node<'_>) -> bool {
    let mut n = syntax::strip_parens(node);
    while kind(n) == "unary_expression" && matches!(syntax::operator_str(n), "-" | "+") {
        match fld(n, F.operand) {
            Some(o) => n = syntax::strip_parens(o),
            None => return false,
        }
    }
    matches!(
        kind(n),
        "decimal_integer_literal"
            | "hex_integer_literal"
            | "octal_integer_literal"
            | "binary_integer_literal"
            | "decimal_floating_point_literal"
            | "hex_floating_point_literal"
            | "character_literal"
            | "string_literal"
            | "true"
            | "false"
            | "null_literal"
    )
}

/// A compile-time constant built only from primitive/String literals and operators
/// (`60 * 1000`, `"a" + "b"`, `-1`, `!true`). Names (even of constants) are not accepted.
pub(super) fn is_literal_constant_expr(node: Node<'_>, depth: u32) -> bool {
    if depth > 32 {
        return false;
    }
    let n = syntax::strip_parens(node);
    match kind(n) {
        "null_literal" => false,
        "binary_expression" => {
            let (Some(l), Some(r)) = (fld(n, F.left), fld(n, F.right)) else {
                return false;
            };
            is_literal_constant_expr(l, depth + 1) && is_literal_constant_expr(r, depth + 1)
        }
        "unary_expression" => {
            fld(n, F.operand).is_some_and(|o| is_literal_constant_expr(o, depth + 1))
        }
        _ => is_literal(n),
    }
}

/// Whether the expression denotes a constant: a literal, a class literal, or a name that
/// reads a `static final` field / enum constant (proven through the resolver, or `UPPER_SNAKE`
/// qualified by a type name such as `Color.RED` or `Limits.MAX`).
pub(super) fn is_constant_like<'t>(ctx: &Ctx<'t>, node: Node<'t>) -> bool {
    let n = syntax::strip_parens(node);
    if is_literal(n) || kind(n) == "class_literal" {
        return true;
    }
    match kind(n) {
        "identifier" => match ctx.declaration_of(n) {
            Some(d) => match d.kind {
                VarKind::EnumConstant => true,
                VarKind::Field => {
                    let m = Mods::of(d.statement);
                    (m.has("static") && m.has("final"))
                        || kind(d.statement) == "constant_declaration"
                }
                _ => false,
            },
            None => false,
        },
        "field_access" => {
            let (Some(obj), Some(field)) = (fld(n, F.object), fld(n, F.field)) else {
                return false;
            };
            if kind(field) != "identifier" || !is_constant_name(ctx.text(field)) {
                return false;
            }
            // `Type.CONST` / `pkg.Type.CONST`: the object names a type.
            matches!(kind(obj), "identifier" | "field_access")
                && ctx
                    .text(obj)
                    .rsplit('.')
                    .next()
                    .is_some_and(|s| s.starts_with(|c: char| c.is_ascii_uppercase()))
                && ctx.type_name_of(obj).is_some()
        }
        _ => false,
    }
}

/// The `true` / `false` value of a boolean literal (parentheses skipped).
pub(super) fn bool_literal(n: Node<'_>) -> Option<bool> {
    match kind(syntax::strip_parens(n)) {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// The members of a type declaration (comments skipped; enum constants and declarations).
pub(super) fn class_members<'a, 't>(
    ctx: &'a Ctx<'t>,
    decl: Node<'t>,
) -> impl Iterator<Item = Node<'t>> + 'a {
    syntax::type_body(decl)
        .into_iter()
        .flat_map(|b| ctx.members(b))
}

/// A code snippet for a message: whitespace collapsed, at most `max` characters (`…` when cut).
pub(super) fn shorten(s: &str, max: usize) -> String {
    let mut out = String::with_capacity(s.len().min(max + 4));
    let mut count = 0usize;
    let mut pending_space = false;
    for c in s.chars() {
        if c.is_whitespace() {
            pending_space = count > 0;
            continue;
        }
        if pending_space {
            out.push(' ');
            count += 1;
            pending_space = false;
        }
        if count + 1 >= max {
            out.push('…');
            return out;
        }
        out.push(c);
        count += 1;
    }
    out
}

/// The `class_body` of an anonymous class creation (`new T() { … }`): the last child of the
/// `object_creation_expression`. Cheap (no cursor).
pub(super) fn anonymous_body(creation: Node<'_>) -> Option<Node<'_>> {
    let n = creation.child_count();
    if n == 0 {
        return None;
    }
    creation.child(n - 1).filter(|c| kind(*c) == "class_body")
}

/// The first named, non-comment child, without creating a tree cursor (cheap on hot paths).
pub(super) fn first_named(node: Node<'_>) -> Option<Node<'_>> {
    let n = node.named_child_count();
    (0..n as u32)
        .filter_map(|i| node.named_child(i))
        .find(|c| !syntax::is_comment(*c))
}

/// The method name of a `method_invocation`, read from the source text right before its
/// argument list — no field lookup, so it is cheap enough to reject most calls on hot paths.
/// Returns `""` for unusual layouts (a comment between the name and `(`); use it only as a
/// pre-filter and read the `name` field for anything else.
pub(super) fn quick_call_name<'t>(ctx: &Ctx<'t>, call: Node<'t>) -> &'t str {
    let n = call.child_count();
    if n < 2 {
        return "";
    }
    let Some(args) = call.child(n - 1) else {
        return "";
    };
    if kind(args) != "argument_list" {
        return "";
    }
    let src = ctx.source();
    let b = src.as_bytes();
    let mut end = args.start_byte().min(b.len());
    while end > 0 && b[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    let mut start = end;
    while start > 0 && is_ident_byte(b[start - 1]) {
        start -= 1;
    }
    src.get(start..end).unwrap_or("")
}

/// The name of a `method_invocation` when it is one of `names`, else `None`. Short calls are
/// first rejected by a text search (no child lookups); long ones (chains, lambda arguments)
/// go straight to [`quick_call_name`], so the cost stays bounded per call.
pub(super) fn call_named<'t>(ctx: &Ctx<'t>, call: Node<'t>, names: &[&str]) -> Option<&'t str> {
    call_named_with(ctx, call, names, names)
}

/// [`call_named`] with separate text pre-filter `needles` (for example one shared prefix).
pub(super) fn call_named_with<'t>(
    ctx: &Ctx<'t>,
    call: Node<'t>,
    needles: &[&str],
    names: &[&str],
) -> Option<&'t str> {
    let text = ctx.text(call);
    if text.len() <= 256 && !needles.iter().any(|n| text.contains(n)) {
        return None;
    }
    let name = quick_call_name(ctx, call);
    names.contains(&name).then_some(name)
}

// ------------------------------------------------------------------------- Lombok

/// Lombok's logger annotations: they add a static logger field and touch nothing else.
pub(super) const LOMBOK_LOGGERS: &[&str] = &[
    "Log",
    "Slf4j",
    "XSlf4j",
    "Log4j",
    "Log4j2",
    "CommonsLog",
    "JBossLog",
    "Flogger",
    "CustomLog",
];

/// Simple names of the Lombok annotations on a declaration: annotations qualified with
/// `lombok.`, or unqualified ones in a file that imports from Lombok.
pub(super) fn lombok_annotations<'a, 't>(
    ctx: &'a Ctx<'t>,
    decl: Node<'t>,
) -> impl Iterator<Item = &'t str> + 'a {
    let imports_lombok = ctx
        .imports()
        .iter()
        .any(|i| i.path == "lombok" || i.path.starts_with("lombok."));
    syntax::annotations(decl).filter_map(move |a| {
        let full = ctx.text(fld(a, F.name)?);
        if let Some(rest) = full.strip_prefix("lombok.") {
            return rest.rsplit('.').next();
        }
        (imports_lombok && !full.contains('.')).then_some(full)
    })
}
