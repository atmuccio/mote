//! Private helpers shared by the batch 7 rules.

use crate::prelude::*;
use crate::rules::batch_6::util::{identifier_kind, ValueKind};

/// The text of the line that contains `offset`, from the start of the line up to `offset`.
pub(super) fn line_prefix<'a>(ctx: &Ctx<'a>, offset: u32) -> &'a str {
    let Some(before) = ctx.source().get(..offset as usize) else {
        return "";
    };
    let start = before.rfind('\n').map_or(0, |i| i + 1);
    before.get(start..).unwrap_or("")
}

/// The leading blanks (spaces and tabs) of `text`.
pub(super) fn leading_blanks(text: &str) -> &str {
    let end = text
        .find(|c: char| c != ' ' && c != '\t')
        .unwrap_or(text.len());
    &text[..end]
}

/// The indentation of the line that contains `offset`.
pub(super) fn indentation_at<'a>(ctx: &Ctx<'a>, offset: u32) -> &'a str {
    leading_blanks(line_prefix(ctx, offset))
}

/// Is `offset` the first non-blank character of its line?
pub(super) fn starts_line(ctx: &Ctx<'_>, offset: u32) -> bool {
    line_prefix(ctx, offset)
        .chars()
        .all(|c| c == ' ' || c == '\t')
}

/// What the scan of the gap between two tokens found.
pub(super) struct Gap {
    /// Offset of the first character that is neither blank, a comment, nor a closing `)`.
    pub at: u32,
    /// That character.
    pub ch: char,
    /// A line break separates it from the last token before it (a closing `)` counts as a token).
    pub newline: bool,
}

/// Scans the source from `start` to `end`, skipping blanks, line breaks, comments and closing
/// parentheses (which end a parenthesized expression), and stops at the first other character.
pub(super) fn scan_gap(ctx: &Ctx<'_>, start: u32, end: u32) -> Option<Gap> {
    let text = ctx.source().get(start as usize..end as usize)?;
    let mut newline = false;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        match c {
            '\n' | '\r' | '\u{2028}' | '\u{2029}' => newline = true,
            c if c.is_whitespace() || c == '\u{feff}' => {}
            ')' => newline = false,
            '/' if matches!(chars.peek(), Some((_, '/'))) => {
                // A line comment runs to the line break, which the loop then sees.
                while chars
                    .peek()
                    .is_some_and(|&(_, c)| !matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}'))
                {
                    chars.next();
                }
            }
            '/' if matches!(chars.peek(), Some((_, '*'))) => {
                chars.next();
                let mut prev = ' ';
                for (_, c) in chars.by_ref() {
                    if matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}') {
                        newline = true;
                    }
                    if prev == '*' && c == '/' {
                        break;
                    }
                    prev = c;
                }
            }
            _ => {
                return Some(Gap {
                    at: start + i as u32,
                    ch: c,
                    newline,
                });
            }
        }
    }
    None
}

/// Is the expression visibly an array: an array literal, `Array.from/of(…)`, `new Array(…)`,
/// `Object.keys/values/entries(…)`, or a variable or parameter declared or initialized as one?
pub(super) fn is_visible_array(ctx: &Ctx<'_>, expr: &Expression<'_>) -> bool {
    match helpers::unwrap_expr(expr) {
        Expression::Identifier(id) => {
            if identifier_kind(ctx, id) == Some(ValueKind::Array) {
                return true;
            }
            // `const keys = Object.keys(o)`, never reassigned.
            let Some(symbol) = ctx.reference_symbol(id) else {
                return false;
            };
            let AstKind::VariableDeclarator(d) = ctx.symbol_declaration(symbol).kind() else {
                return false;
            };
            matches!(d.id, BindingPattern::BindingIdentifier(_))
                && d.type_annotation.is_none()
                && d.init
                    .as_ref()
                    .is_some_and(|init| is_array_value(ctx, init))
                && ctx.scoping().symbol_redeclarations(symbol).is_empty()
                && !ctx.references_of(symbol).any(|r| r.is_write())
        }
        other => is_array_value(ctx, other),
    }
}

/// An array literal, `new Array(…)`, `Array.from/of(…)`, `Object.keys/values/entries(…)`.
fn is_array_value(ctx: &Ctx<'_>, expr: &Expression<'_>) -> bool {
    match helpers::unwrap_expr(expr) {
        Expression::ArrayExpression(_) => true,
        Expression::NewExpression(new) => ctx.is_global(&new.callee, "Array"),
        Expression::CallExpression(call) => is_array_factory(ctx, call),
        _ => false,
    }
}

/// `Array.from(…)`, `Array.of(…)`, `Object.keys/values/entries(…)` on the globals.
pub(super) fn is_array_factory(ctx: &Ctx<'_>, call: &CallExpression<'_>) -> bool {
    let Expression::StaticMemberExpression(m) = helpers::unwrap_expr(&call.callee) else {
        return false;
    };
    match m.property.name.as_str() {
        "from" | "of" => ctx.is_global(&m.object, "Array"),
        "keys" | "values" | "entries" => ctx.is_global(&m.object, "Object"),
        _ => false,
    }
}

/// A function or arrow function node (their subtrees are skipped when scanning the body of
/// another function or loop).
pub(super) fn is_function_node(kind: AstKind<'_>) -> bool {
    matches!(
        kind,
        AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
    )
}
