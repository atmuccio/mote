//! Helpers of the batch-9 rules, also used by batch 14 and by [`super::regex_call`]: a per-file
//! text gate, call arguments, receiver tests and constant folding of string literals. Nothing
//! here walks the whole tree.

use crate::prelude::*;

/// Per-file "does the source text contain `needle`?", computed on first use. Lets a rule skip
/// resolver work in the (many) files that cannot contain what it looks for.
#[derive(Default)]
pub(crate) struct Gate(Option<bool>);

impl Gate {
    pub(crate) fn open(&mut self, ctx: &Ctx<'_>, needle: &str) -> bool {
        *self.0.get_or_insert_with(|| ctx.source().contains(needle))
    }
}

/// The argument expressions of a call or object creation (comments skipped).
pub(crate) fn args(call: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    call.field(Field::Arguments)
        .into_iter()
        .flat_map(named_children)
}

/// Number of arguments of a call or object creation.
pub(crate) fn arg_count(call: Node<'_>) -> usize {
    args(call).count()
}

/// The `i`-th argument of a call or object creation.
pub(crate) fn arg(call: Node<'_>, i: usize) -> Option<Node<'_>> {
    args(call).nth(i)
}

/// Whether `expr` names the type `fqn` (the receiver of a static call like `Pattern.compile`).
pub(crate) fn names_type<'t>(ctx: &Ctx<'t>, expr: Node<'t>, fqn: &str) -> bool {
    let simple = fqn.rsplit('.').next().unwrap_or(fqn);
    let text = ctx.text(expr);
    if text != simple && text != fqn {
        return false;
    }
    ctx.type_name_of(expr).as_deref() == Some(fqn)
}

/// Whether the receiver of `call` is provably a `String` (a string literal or an expression the
/// resolver types as `String`).
pub(crate) fn receiver_is_string<'t>(ctx: &Ctx<'t>, call: Node<'t>) -> bool {
    let Some(obj) = call.field(Field::Object) else {
        return false;
    };
    let obj = syntax::strip_parens(obj);
    kind(obj) == "string_literal" || ctx.type_of(obj).is_some_and(|t| t.is_string())
}

/// The runtime value of a string constant made of plain string literals, possibly joined with
/// `+` and wrapped in parentheses. `None` for text blocks, Unicode escapes, and anything that
/// is not a literal.
pub(crate) fn string_constant(ctx: &Ctx<'_>, node: Node<'_>) -> Option<String> {
    // Walk the left spine of `a + b + c` iteratively (long concatenations are deep trees).
    let mut parts: Vec<Node<'_>> = Vec::new();
    let mut cur = syntax::strip_parens(node);
    loop {
        match kind(cur) {
            "string_literal" => {
                parts.push(cur);
                break;
            }
            "binary_expression" if syntax::operator_str(cur) == "+" => {
                let (Some(l), Some(r)) = (cur.field(Field::Left), cur.field(Field::Right)) else {
                    return None;
                };
                parts.push(r);
                cur = syntax::strip_parens(l);
            }
            _ => return None,
        }
        if parts.len() > 256 {
            return None;
        }
    }
    let mut out = String::new();
    for p in parts.iter().rev() {
        let p = syntax::strip_parens(*p);
        if kind(p) != "string_literal" {
            // A nested concatenation on the right (`"a" + ("b" + "c")`) is folded recursively.
            if kind(p) == "binary_expression" {
                out.push_str(&string_constant(ctx, p)?);
                continue;
            }
            return None;
        }
        out.push_str(&string_value(ctx, p)?);
    }
    Some(out)
}

/// A plain (non-text-block) string literal decoded to its runtime value.
fn string_value(ctx: &Ctx<'_>, node: Node<'_>) -> Option<String> {
    let text = ctx.text(node);
    if text.starts_with("\"\"\"") {
        return None;
    }
    let raw = syntax::string_literal_content(node, ctx.source())?;
    unescape(raw)
}

/// Decode the escape sequences of Java string-literal source text. `None` on Unicode escapes
/// (they are processed before tokenization, so the raw text may not mean what it looks like)
/// or unknown escapes.
pub(crate) fn unescape(raw: &str) -> Option<String> {
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

/// Pre-order visit of the **named** descendants of `root` (not `root` itself), in source order,
/// not entering nodes for which `enter` is false. Linear in the size of the subtree.
pub(crate) fn visit<'t>(
    root: Node<'t>,
    mut enter: impl FnMut(Node<'t>) -> bool,
    mut f: impl FnMut(Node<'t>),
) {
    let mut cursor = root.walk();
    if !cursor.goto_first_child() {
        return;
    }
    loop {
        let n = cursor.node();
        let descend = if n.is_named() {
            f(n);
            enter(n)
        } else {
            false
        };
        if descend && cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() || cursor.node().id() == root.id() {
                return;
            }
        }
    }
}

/// The byte ranges of the subtrees a lazily built per-file index has covered so far, for rules
/// that index the whole subtree of the first node needing it (a loop body, a block) and answer
/// later questions about nested nodes from the same index: indexing the whole file up front
/// visits every node of files where only one small region matters, and indexing every node's
/// own subtree is quadratic in the nesting. The ranges are subtrees, so a new one either lies
/// inside a range added before or is disjoint from all of them; they are kept sorted.
#[derive(Default)]
pub(crate) struct CoveredRanges(Vec<(usize, usize)>);

impl CoveredRanges {
    /// Whether `[from, to)` lies inside a range added before.
    pub(crate) fn contains(&self, from: usize, to: usize) -> bool {
        let i = self.0.partition_point(|r| r.0 <= from);
        i > 0 && self.0.get(i - 1).is_some_and(|r| to <= r.1)
    }

    /// Adds `[from, to)` (a subtree disjoint from the ranges added before). Returns whether it
    /// lies after all of them: then index entries in source order stay sorted when appended.
    pub(crate) fn add(&mut self, from: usize, to: usize) -> bool {
        let i = self.0.partition_point(|r| r.0 <= from);
        self.0.insert(i, (from, to));
        i + 1 == self.0.len()
    }
}

/// The last segment of a dotted name (`java.lang.Integer` → `Integer`).
pub(crate) fn last_segment(text: &str) -> &str {
    text.rsplit('.').next().unwrap_or(text).trim()
}

#[cfg(test)]
mod tests {
    use super::unescape;

    #[test]
    fn unescapes_java_strings() {
        assert_eq!(unescape("a\\\\b").as_deref(), Some("a\\b"));
        assert_eq!(unescape("\\t\\n").as_deref(), Some("\t\n"));
        assert_eq!(unescape("\\bword").as_deref(), Some("\u{8}word"));
        assert_eq!(unescape("\\101").as_deref(), Some("A"));
        assert_eq!(unescape("\\u0041"), None);
        assert_eq!(unescape("\\q"), None);
    }
}
