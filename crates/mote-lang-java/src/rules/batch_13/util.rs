//! Helpers shared by the batch-13 rules (maintainability smells): call arguments, receiver types,
//! string constants and a few declaration shapes. Nothing here walks the whole tree: every function
//! looks at one node, its children, or one bounded subtree.

use crate::prelude::*;

// ------------------------------------------------------------------------- per-file gate

/// Per-file "does the source text contain `needle`?", computed on first use. Lets a rule skip
/// resolver work in the (many) files that cannot contain what it looks for.
#[derive(Default)]
pub(super) struct Gate(Option<bool>);

impl Gate {
    pub(super) fn open(&mut self, ctx: &Ctx<'_>, needle: &str) -> bool {
        *self.0.get_or_insert_with(|| ctx.source().contains(needle))
    }
}

// ------------------------------------------------------------------------- calls

/// The argument expressions of a call or object creation (comments skipped).
pub(super) fn args(call: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    call.field(Field::Arguments)
        .into_iter()
        .flat_map(named_children)
}

/// Number of arguments of a call or object creation.
pub(super) fn arg_count(call: Node<'_>) -> usize {
    args(call).count()
}

/// The `i`-th argument of a call or object creation.
pub(super) fn arg(call: Node<'_>, i: usize) -> Option<Node<'_>> {
    args(call).nth(i)
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

/// Whether the static type of `expr` is provably a subtype of `fqn`.
pub(super) fn is_a<'t>(ctx: &Ctx<'t>, expr: Node<'t>, fqn: &str) -> bool {
    ctx.type_of(expr)
        .is_some_and(|t| ctx.is_subtype_of(&t, fqn))
}

/// The last segment of a dotted name (`java.lang.Integer` → `Integer`).
pub(super) fn last_segment(text: &str) -> &str {
    text.rsplit('.').next().unwrap_or(text).trim()
}

// ------------------------------------------------------------------------- declarations

/// The simple names of the annotations of a declaration (`@a.b.Nullable` → `Nullable`).
pub(super) fn annotation_names<'t>(
    ctx: &Ctx<'t>,
    decl: Node<'t>,
) -> impl Iterator<Item = (Node<'t>, &'t str)> {
    let src = ctx.source();
    syntax::annotations(decl).map(move |a| (a, last_segment(syntax::annotation_name(a, src))))
}

/// Whether a comment of its own (Javadoc, block or line comment) sits right above `decl`: a
/// sign that the member is there on purpose. A trailing comment of the previous member does
/// not count.
pub(super) fn has_leading_comment<'t>(ctx: &Ctx<'t>, decl: Node<'t>) -> bool {
    let Some(p) = ctx.prev_named_sibling(decl) else {
        return false;
    };
    if !syntax::is_comment(p) || p.end_position().row + 1 < decl.start_position().row {
        return false;
    }
    ctx.prev_named_sibling(p)
        .is_none_or(|before| before.end_position().row < p.start_position().row)
}

/// The statements of a block, comments skipped.
pub(super) fn statements(block: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    named_children(block).filter(|n| !syntax::is_comment(*n))
}

/// The only statement of a block (comments skipped), if it has exactly one.
pub(super) fn single_statement(block: Node<'_>) -> Option<Node<'_>> {
    let mut it = statements(block);
    let first = it.next()?;
    it.next().is_none().then_some(first)
}

/// Whether `text` contains `word` delimited by non-identifier characters.
pub(super) fn contains_word(text: &str, word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    let is_ident = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    let mut from = 0;
    while let Some(pos) = text[from..].find(word) {
        let start = from + pos;
        let end = start + word.len();
        let before_ok = text[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !is_ident(c));
        let after_ok = text[end..].chars().next().is_none_or(|c| !is_ident(c));
        if before_ok && after_ok {
            return true;
        }
        from = end;
    }
    false
}

// ------------------------------------------------------------------------- string constants

/// The runtime value of a string constant: plain string literals, possibly joined with `+` and
/// wrapped in parentheses, and names of `static final String` fields of this file initialized
/// that way. `None` for text blocks and anything else.
pub(super) fn string_constant<'t>(ctx: &Ctx<'t>, node: Node<'t>, depth: u32) -> Option<String> {
    if depth > 8 {
        return None;
    }
    let node = syntax::strip_parens(node);
    match kind(node) {
        "string_literal" => {
            if ctx.text(node).starts_with("\"\"\"") {
                return None;
            }
            unescape(syntax::string_literal_content(node, ctx.source())?)
        }
        "binary_expression" if syntax::operator_str(node) == "+" => {
            let l = string_constant(ctx, node.field(Field::Left)?, depth + 1)?;
            let r = string_constant(ctx, node.field(Field::Right)?, depth + 1)?;
            Some(l + &r)
        }
        "identifier" => {
            let decl = ctx.declaration_of(node)?;
            if decl.kind != VarKind::Field {
                return None;
            }
            let stmt = decl.statement;
            if !syntax::has_modifier(stmt, "static") || !syntax::has_modifier(stmt, "final") {
                return None;
            }
            string_constant(ctx, decl.value?, depth + 1)
        }
        _ => None,
    }
}

/// Decodes the escapes of a Java string literal's content. `None` for Unicode escapes and
/// malformed escapes.
fn unescape(s: &str) -> Option<String> {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
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
            '0'..='7' => {
                let mut v = e.to_digit(8)?;
                let max_digits = if e <= '3' { 3 } else { 2 };
                let mut n = 1;
                while n < max_digits {
                    match it.peek().and_then(|d| d.to_digit(8)) {
                        Some(d) => {
                            v = v * 8 + d;
                            it.next();
                            n += 1;
                        }
                        None => break,
                    }
                }
                out.push(char::from_u32(v)?);
            }
            '\\' | '\'' | '"' => out.push(e),
            _ => return None,
        }
    }
    Some(out)
}
