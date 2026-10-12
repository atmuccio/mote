//! Private helpers shared by the batch-2 rules (source-text lookups the frozen API does not offer).

use crate::prelude::*;

/// Is the byte `offset` inside a comment (delimiters included)?
pub fn in_comment(ctx: &Ctx<'_>, offset: u32) -> bool {
    let comments = ctx.comments();
    let i = comments.partition_point(|c| c.span.end <= offset);
    comments.get(i).is_some_and(|c| c.span.start <= offset)
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$' || b >= 0x80
}

/// Span of the first occurrence of `token` inside `gap` that is not in a comment. Alphabetic
/// tokens (keywords) must be whole words.
pub fn find_token(ctx: &Ctx<'_>, gap: Span, token: &str) -> Option<Span> {
    let text = ctx.text(gap);
    let bytes = text.as_bytes();
    let word = token.bytes().all(|b| b.is_ascii_alphabetic());
    let mut from = 0;
    while let Some(pos) = text.get(from..).and_then(|t| t.find(token)) {
        let at = from + pos;
        let end = at + token.len();
        let bounded = !word
            || ((at == 0 || !is_ident_byte(bytes[at - 1]))
                && bytes.get(end).is_none_or(|&b| !is_ident_byte(b)));
        let start = gap.start + at as u32;
        if bounded && !in_comment(ctx, start) {
            return Some(Span::new(start, start + token.len() as u32));
        }
        from = end;
    }
    None
}

/// Nearest non-whitespace byte before `offset`, skipping comments. `None` at the file start.
fn prev_significant(ctx: &Ctx<'_>, offset: u32) -> Option<u8> {
    let bytes = ctx.source().as_bytes();
    let comments = ctx.comments();
    let mut i = offset as usize;
    while i > 0 {
        let b = *bytes.get(i - 1)?;
        if b.is_ascii_whitespace() {
            i -= 1;
            continue;
        }
        // A comment ending here: jump to its start.
        let at = (i - 1) as u32;
        let ci = comments.partition_point(|c| c.span.end <= at);
        match comments.get(ci) {
            Some(c) if c.span.start <= at => i = c.span.start as usize,
            _ => return Some(b),
        }
    }
    None
}

/// Nearest non-whitespace byte at or after `offset`, skipping comments.
fn next_significant(ctx: &Ctx<'_>, offset: u32) -> Option<u8> {
    let bytes = ctx.source().as_bytes();
    let comments = ctx.comments();
    let mut i = offset as usize;
    while let Some(&b) = bytes.get(i) {
        if b.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let at = i as u32;
        let ci = comments.partition_point(|c| c.span.end <= at);
        match comments.get(ci) {
            Some(c) if c.span.start <= at => i = c.span.end as usize,
            _ => return Some(b),
        }
    }
    None
}

/// Is the expression at `span` written inside its own pair of parentheses, `( … )`? The parser
/// drops parentheses, so this looks at the source text around the span.
pub fn is_parenthesized(ctx: &Ctx<'_>, span: Span) -> bool {
    prev_significant(ctx, span.start) == Some(b'(') && next_significant(ctx, span.end) == Some(b')')
}

/// A script that uses CommonJS (`require`, `module`, `exports`) has a module-private top level.
pub fn has_private_top_level(ctx: &Ctx<'_>) -> bool {
    ctx.is_module()
        || ctx.source_type().is_commonjs()
        || ctx
            .scoping()
            .root_unresolved_references()
            .contains_key("require")
        || ctx
            .scoping()
            .root_unresolved_references()
            .contains_key("module")
        || ctx
            .scoping()
            .root_unresolved_references()
            .contains_key("exports")
}

/// Is `name` referenced as an unresolved global anywhere in the file?
pub fn uses_global(ctx: &Ctx<'_>, name: &str) -> bool {
    ctx.scoping()
        .root_unresolved_references()
        .contains_key(name)
}

/// Is the expression a string literal or a template literal (with or without substitutions)?
pub fn is_string_literal(expr: &Expression<'_>) -> bool {
    matches!(
        helpers::unwrap_expr(expr),
        Expression::StringLiteral(_) | Expression::TemplateLiteral(_)
    )
}

/// A short, single-line preview of a string for messages.
pub fn preview(text: &str, max: usize) -> String {
    let mut out: String = text
        .chars()
        .take(max)
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    if text.chars().nth(max).is_some() {
        out.push('…');
    }
    out
}

/// The node's parent after skipping TypeScript expression wrappers (`as`, `satisfies`, `!`,
/// `<T>x`), together with the span of the outermost wrapper (or the node itself).
pub fn parent_through_wrappers<'a>(
    ctx: &Ctx<'a>,
    node: &AstNode<'a>,
) -> Option<(&'a AstNode<'a>, Span)> {
    let mut span = node.span();
    for ancestor in ctx.ancestors(node) {
        match ancestor.kind() {
            AstKind::TSAsExpression(_)
            | AstKind::TSSatisfiesExpression(_)
            | AstKind::TSNonNullExpression(_)
            | AstKind::TSTypeAssertion(_)
            | AstKind::ParenthesizedExpression(_) => span = ancestor.span(),
            _ => return Some((ancestor, span)),
        }
    }
    None
}

/// Is the code at `offset` directly preceded by a comment (only whitespace in between)?
pub fn has_comment_before(ctx: &Ctx<'_>, offset: u32) -> bool {
    let comments = ctx.comments();
    let i = comments.partition_point(|c| c.span.end <= offset);
    i > 0
        && comments.get(i - 1).is_some_and(|c| {
            ctx.text(Span::new(c.span.end, offset))
                .bytes()
                .all(|b| b.is_ascii_whitespace())
        })
}
