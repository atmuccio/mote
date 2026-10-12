//! Structural helpers over tree-sitter-java nodes. None of these walk the whole tree.
//!
//! Comments are *extra* nodes that tree-sitter may attach anywhere, so positional access
//! (`node.named_child(0)`) can return a comment. Prefer field access
//! (`node.child_by_field_name("body")`) or [`named_children`], which skips comments.

use crate::kinds::{k, kind, FIELD_IDS, K};
use tree_sitter::{Node, TreeCursor};

// ------------------------------------------------------------------------------ fields

/// A grammar field of tree-sitter-java. `syntax::field(node, Field::Name)` (or
/// `node.field(Field::Name)` with [`NodeExt`]) is `node.child_by_field_name("name")` without
/// the by-name lookup, which scans the grammar's field table on every call.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Field {
    Alternative,
    Arguments,
    Array,
    Body,
    Condition,
    Consequence,
    Constructor,
    Declarator,
    Dimensions,
    Element,
    Field,
    Index,
    Init,
    Interfaces,
    Key,
    Left,
    Modifiers,
    Name,
    Object,
    Operand,
    Operator,
    Parameters,
    Pattern,
    Permits,
    Resources,
    Right,
    Scope,
    Superclass,
    Type,
    TypeArguments,
    TypeParameters,
    Update,
    Value,
}

pub(crate) const FIELD_COUNT: usize = 33;

/// Grammar names of [`Field`], in declaration order.
pub(crate) const FIELD_NAMES: [&str; FIELD_COUNT] = [
    "alternative",
    "arguments",
    "array",
    "body",
    "condition",
    "consequence",
    "constructor",
    "declarator",
    "dimensions",
    "element",
    "field",
    "index",
    "init",
    "interfaces",
    "key",
    "left",
    "modifiers",
    "name",
    "object",
    "operand",
    "operator",
    "parameters",
    "pattern",
    "permits",
    "resources",
    "right",
    "scope",
    "superclass",
    "type",
    "type_arguments",
    "type_parameters",
    "update",
    "value",
];

impl Field {
    /// The grammar's name of the field (`"name"`, `"type_arguments"`).
    pub fn name(self) -> &'static str {
        FIELD_NAMES[self as usize]
    }
}

/// The child in field `f` — same result as `node.child_by_field_name(f.name())`, faster.
#[inline]
pub fn field(node: Node<'_>, f: Field) -> Option<Node<'_>> {
    match FIELD_IDS[f as usize] {
        0 => None,
        id => node.child_by_field_id(id),
    }
}

/// Faster node accessors as methods: `node.field(Field::Name)`.
pub trait NodeExt<'t> {
    /// The child in field `f` (see [`field`]).
    fn field(self, f: Field) -> Option<Node<'t>>;
}

impl<'t> NodeExt<'t> for Node<'t> {
    #[inline]
    fn field(self, f: Field) -> Option<Node<'t>> {
        field(self, f)
    }
}

// ------------------------------------------------------------------------------ children

/// Lists up to this long are read by index (`child(i)` / `named_child(i)` cost O(i) but
/// allocate nothing); longer ones with a tree cursor (one allocation, O(1) per step).
const SHORT: u32 = 8;

/// Iterator over a node's children (see [`children`] / [`named_children`]). Short lists are
/// read by index, long ones with a tree cursor; the sequence is the same either way.
pub struct Children<'t> {
    node: Node<'t>,
    named: bool,
    state: State<'t>,
}

enum State<'t> {
    /// Next index and count (`named_child` indices when `named`).
    Index(u32, u32),
    /// Cursor walk: `false` before the first child.
    Cursor(TreeCursor<'t>, bool),
    Done,
}

impl<'t> Children<'t> {
    fn new(node: Node<'t>, named: bool) -> Self {
        let n = if named {
            node.named_child_count() as u32
        } else {
            node.child_count()
        };
        let state = if n == 0 {
            State::Done
        } else if n <= SHORT {
            State::Index(0, n)
        } else {
            State::Cursor(node.walk(), false)
        };
        Children { node, named, state }
    }
}

impl<'t> Iterator for Children<'t> {
    type Item = Node<'t>;
    #[inline]
    fn next(&mut self) -> Option<Node<'t>> {
        loop {
            let n = match &mut self.state {
                State::Done => return None,
                State::Index(i, count) => {
                    if *i >= *count {
                        self.state = State::Done;
                        return None;
                    }
                    let c = if self.named {
                        self.node.named_child(*i)
                    } else {
                        self.node.child(*i)
                    };
                    *i += 1;
                    match c {
                        Some(c) => c,
                        None => continue,
                    }
                }
                State::Cursor(cursor, started) => {
                    let moved = if *started {
                        cursor.goto_next_sibling()
                    } else {
                        *started = true;
                        cursor.goto_first_child()
                    };
                    if !moved {
                        self.state = State::Done;
                        return None;
                    }
                    cursor.node()
                }
            };
            if !self.named || (n.is_named() && !is_comment(n)) {
                return Some(n);
            }
        }
    }
}

/// All children, including anonymous tokens (`(`, `static`, `==`) and comments.
pub fn children(node: Node<'_>) -> Children<'_> {
    Children::new(node, false)
}

/// Named children, **excluding comments**.
pub fn named_children(node: Node<'_>) -> Children<'_> {
    Children::new(node, true)
}

/// The first named, non-comment child. Never allocates.
pub fn first_named_child(node: Node<'_>) -> Option<Node<'_>> {
    let n = node.named_child_count() as u32;
    (0..n)
        .filter_map(|i| node.named_child(i))
        .find(|c| !is_comment(*c))
}

/// The first child (named or not) of the given kind.
pub fn child_of_kind<'t>(node: Node<'t>, kind_name: &str) -> Option<Node<'t>> {
    children(node).find(|c| kind(*c) == kind_name)
}

/// `line_comment` or `block_comment`.
#[inline]
pub fn is_comment(node: Node<'_>) -> bool {
    matches!(k(node), K::LineComment | K::BlockComment)
}

/// Source text of `node` (empty on out-of-range offsets; never panics).
#[inline]
pub fn node_text<'s>(node: Node<'_>, src: &'s str) -> &'s str {
    src.get(node.start_byte()..node.end_byte()).unwrap_or("")
}

/// Skip any number of enclosing `(` `)`.
pub fn strip_parens(mut node: Node<'_>) -> Node<'_> {
    while k(node) == K::ParenthesizedExpression {
        match first_named_child(node) {
            Some(inner) => node = inner,
            None => break,
        }
    }
    node
}

/// The `modifiers` child of a declaration, if any.
pub fn modifiers(decl: Node<'_>) -> Option<Node<'_>> {
    // Modifiers come first in every declaration: check child 0 before scanning.
    let first = decl.child(0)?;
    if k(first) == K::Modifiers {
        return Some(first);
    }
    children(decl).find(|c| k(*c) == K::Modifiers)
}

/// Whether the declaration has the keyword modifier `kw` (`"static"`, `"final"`, `"private"`,
/// `"abstract"`, `"default"`, `"synchronized"`, …).
pub fn has_modifier(decl: Node<'_>, kw: &str) -> bool {
    modifiers(decl).is_some_and(|m| children(m).any(|c| !c.is_named() && kind(c) == kw))
}

/// Annotations (`annotation` / `marker_annotation`) on a declaration: those inside its
/// `modifiers` plus any placed directly on it (e.g. after method type parameters).
pub fn annotations(decl: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    let in_mods = modifiers(decl).into_iter().flat_map(named_children);
    let direct = named_children(decl);
    in_mods
        .chain(direct)
        .filter(|n| matches!(k(*n), K::Annotation | K::MarkerAnnotation))
}

/// Simple name of an annotation node (`Override` for `@java.lang.Override`).
pub fn annotation_name<'s>(ann: Node<'_>, src: &'s str) -> &'s str {
    let Some(name) = field(ann, Field::Name) else {
        return "";
    };
    let full = node_text(name, src);
    full.rsplit('.').next().unwrap_or(full).trim()
}

/// Whether `decl` carries an annotation whose simple name is `name`.
pub fn has_annotation(decl: Node<'_>, src: &str, name: &str) -> bool {
    annotations(decl).any(|a| annotation_name(a, src) == name)
}

/// Class-like declarations: class, interface, enum, record, annotation type.
pub fn is_type_declaration(node: Node<'_>) -> bool {
    k(node).is_type_decl()
}

/// Method, constructor, or compact constructor declaration.
pub fn is_method_like(node: Node<'_>) -> bool {
    matches!(
        k(node),
        K::MethodDecl | K::ConstructorDecl | K::CompactConstructorDecl
    )
}

/// Method-like declarations, lambdas, and static initializers. (Instance initializer blocks are
/// plain `block`s directly inside a class body; [`crate::Ctx::enclosing_function`] handles them.)
pub fn is_function_like(node: Node<'_>) -> bool {
    matches!(
        k(node),
        K::MethodDecl
            | K::ConstructorDecl
            | K::CompactConstructorDecl
            | K::Lambda
            | K::StaticInitializer
    )
}

/// The body node (`class_body`, `interface_body`, `enum_body`, `annotation_type_body`) of a
/// type declaration, an anonymous class (`object_creation_expression`), or an enum constant.
pub fn type_body(decl: Node<'_>) -> Option<Node<'_>> {
    match k(decl) {
        K::ObjectCreation | K::EnumConstant => decl
            .field(Field::Body)
            .or_else(|| children(decl).find(|c| k(*c) == K::ClassBody)),
        _ => field(decl, Field::Body),
    }
}

/// Members of a type body. For enums this yields the constants and then the declarations in
/// `enum_body_declarations`. Comments are skipped.
pub fn members(body: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    named_children(body).flat_map(|c| {
        let nested = (k(c) == K::EnumBodyDecls).then_some(c);
        let own = (k(c) != K::EnumBodyDecls).then_some(c);
        own.into_iter()
            .chain(nested.into_iter().flat_map(named_children))
    })
}

/// The `variable_declarator`s of a field / local / constant declaration.
pub fn declarators(decl: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    named_children(decl).filter(|c| kind(*c) == "variable_declarator")
}

/// The formal parameters (`formal_parameter` and `spread_parameter`, not the receiver
/// parameter) of a method/constructor/lambda with a `formal_parameters` list.
pub fn parameters(decl: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    field(decl, Field::Parameters)
        .filter(|p| kind(*p) == "formal_parameters")
        .into_iter()
        .flat_map(named_children)
        .filter(|p| matches!(kind(*p), "formal_parameter" | "spread_parameter"))
}

/// The argument expressions of a `method_invocation`, `object_creation_expression`, or
/// `explicit_constructor_invocation`.
pub fn arguments(call: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    field(call, Field::Arguments)
        .into_iter()
        .flat_map(named_children)
}

/// Number of arguments of a call.
pub fn argument_count(call: Node<'_>) -> usize {
    arguments(call).count()
}

/// The operator token of a `binary_expression`, `unary_expression`, or `assignment_expression`.
pub fn operator(expr: Node<'_>) -> Option<Node<'_>> {
    field(expr, Field::Operator)
}

/// The operator of `expr` as a string (`"=="`, `"&&"`, `"+="`), or `""`.
pub fn operator_str(expr: Node<'_>) -> &'static str {
    operator(expr).map(kind).unwrap_or("")
}

/// Whether the node is the `null` literal (parentheses are skipped).
pub fn is_null_literal(node: Node<'_>) -> bool {
    kind(strip_parens(node)) == "null_literal"
}

/// Whether the node is a string literal or text block (parentheses are skipped).
pub fn is_string_literal(node: Node<'_>) -> bool {
    kind(strip_parens(node)) == "string_literal"
}

/// Whether the node is an integer literal with the given value (`0`, `0L`, `0x0`, `1_0`).
pub fn is_int_literal(node: Node<'_>, src: &str, value: i64) -> bool {
    int_literal_value(node, src) == Some(value)
}

/// The value of a decimal/hex/octal/binary integer literal (parentheses skipped; a leading
/// unary minus is honoured).
pub fn int_literal_value(node: Node<'_>, src: &str) -> Option<i64> {
    let mut node = strip_parens(node);
    let mut negate = false;
    while kind(node) == "unary_expression" && operator_str(node) == "-" {
        negate = !negate;
        node = strip_parens(field(node, Field::Operand)?);
    }
    let value = literal_value(node, src)?;
    Some(if negate { value.wrapping_neg() } else { value })
}

fn literal_value(node: Node<'_>, src: &str) -> Option<i64> {
    // Kind first: most callers pass arbitrary expressions.
    let (radix, prefix) = match kind(node) {
        "decimal_integer_literal" => (10, 0),
        "hex_integer_literal" => (16, 2),
        "octal_integer_literal" => (8, 1),
        "binary_integer_literal" => (2, 2),
        _ => return None,
    };
    let text = node_text(node, src);
    let owned: String;
    let clean = if text.contains('_') {
        owned = text.chars().filter(|c| *c != '_').collect();
        owned.as_str()
    } else {
        text
    };
    let clean = clean.trim_end_matches(['l', 'L']);
    if radix == 10 {
        clean.parse().ok()
    } else {
        i64::from_str_radix(clean.get(prefix..)?, radix).ok()
    }
}

/// The content of a plain string literal without quotes and without unescaping
/// (`"a\n"` → `a\n`). Text blocks return their raw content between the `"""` delimiters.
pub fn string_literal_content<'s>(node: Node<'_>, src: &'s str) -> Option<&'s str> {
    let node = strip_parens(node);
    if kind(node) != "string_literal" {
        return None;
    }
    let t = node_text(node, src);
    if t.len() >= 6 && t.starts_with("\"\"\"") && t.ends_with("\"\"\"") {
        return t.get(3..t.len() - 3);
    }
    if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
        return t.get(1..t.len() - 1);
    }
    None
}

/// The name identifier of a declaration (`name` field), for methods, classes, fields'
/// declarators, parameters, …
pub fn name_node(decl: Node<'_>) -> Option<Node<'_>> {
    field(decl, Field::Name)
}

/// The name text of a declaration, or `""`.
pub fn name_of<'s>(decl: Node<'_>, src: &'s str) -> &'s str {
    name_node(decl).map(|n| node_text(n, src)).unwrap_or("")
}

/// Whether the statement is an empty block `{}` (comments inside are ignored — use
/// [`block_has_comment`] to check for them).
pub fn is_empty_block(block: Node<'_>) -> bool {
    named_children(block).next().is_none()
}

/// Whether any comment is a direct child of the block.
pub fn block_has_comment(block: Node<'_>) -> bool {
    children(block).any(is_comment)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{parse, ParseBudget};

    #[test]
    fn helpers_work() {
        let src = "class A { @Deprecated private static final int X = 0x1F; void m(int a, String... b) { f(1, 2); } }";
        let tree = parse(src, ParseBudget::DEFAULT, None).unwrap();
        let class = first_named_child(tree.root_node()).unwrap();
        let body = type_body(class).unwrap();
        let ms: Vec<_> = members(body).collect();
        assert_eq!(ms.len(), 2);
        let field = ms[0];
        assert!(has_modifier(field, "static"));
        assert!(has_modifier(field, "private"));
        assert!(!has_modifier(field, "public"));
        assert!(has_annotation(field, src, "Deprecated"));
        let decl = declarators(field).next().unwrap();
        assert_eq!(name_of(decl, src), "X");
        assert_eq!(
            int_literal_value(decl.child_by_field_name("value").unwrap(), src),
            Some(31)
        );
        let method = ms[1];
        assert_eq!(parameters(method).count(), 2);
    }
}
