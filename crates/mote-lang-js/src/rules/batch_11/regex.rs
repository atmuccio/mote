//! Parsing regular expression literals for the batch 11 regex rules (the batch 7 and 8 helpers
//! are private to their batches).

use crate::prelude::*;
use oxc_allocator::Allocator;
use oxc_regular_expression::ast::Pattern;
use oxc_regular_expression::{LiteralParser, Options};

/// Deepest group nesting the rules parse and walk.
pub(super) const MAX_DEPTH: usize = 64;

/// The flags text of a regular expression literal (`gi` in `/x/gi`).
pub(super) fn flags_text<'a>(ctx: &Ctx<'a>, lit: &RegExpLiteral<'a>) -> &'a str {
    let text = ctx.text(lit.span);
    match text.rfind('/') {
        Some(i) => text.get(i + 1..).unwrap_or(""),
        None => "",
    }
}

/// Parses the pattern of a regular expression literal; spans in the result are offsets in the
/// file. `None` when the pattern does not parse (`js/invalid-regexp` reports those) or nests
/// deeper than [`MAX_DEPTH`].
pub(super) fn parse<'b>(
    allocator: &'b Allocator,
    ctx: &Ctx<'b>,
    lit: &RegExpLiteral<'b>,
) -> Option<Pattern<'b>> {
    let pattern = lit.regex.pattern.text.as_str();
    if !crate::helpers::regex_within_limits(pattern, MAX_DEPTH) {
        return None;
    }
    let flags = flags_text(ctx, lit);
    let options = Options {
        pattern_span_offset: lit.span.start + 1,
        flags_span_offset: lit.span.end.saturating_sub(flags.len() as u32),
    };
    LiteralParser::new(allocator, pattern, Some(flags), options)
        .parse()
        .ok()
}
