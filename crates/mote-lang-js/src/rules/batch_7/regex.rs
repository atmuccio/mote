//! Parsing and walking regular expression literals for the batch 7 regex rules.

use crate::prelude::*;
use oxc_allocator::Allocator;
use oxc_regular_expression::ast::{
    CharacterClass, CharacterClassContents, Disjunction, Pattern, Term,
};
use oxc_regular_expression::{LiteralParser, Options};

/// Deepest group nesting the walkers follow.
const MAX_DEPTH: usize = 64;

/// The flags text of a regular expression literal (`gi` in `/x/gi`).
pub(super) fn flags_text<'a>(ctx: &Ctx<'a>, lit: &RegExpLiteral<'a>) -> &'a str {
    let text = ctx.text(lit.span);
    match text.rfind('/') {
        Some(i) => text.get(i + 1..).unwrap_or(""),
        None => "",
    }
}

/// Is the literal in Unicode (`u`) or Unicode-sets (`v`) mode?
pub(super) fn is_unicode(lit: &RegExpLiteral<'_>) -> bool {
    lit.regex.flags.intersects(RegExpFlags::U | RegExpFlags::V)
}

/// Parses the pattern of a regular expression literal. Spans in the result are offsets in the
/// file. `None` when the pattern does not parse (another rule reports invalid patterns) or nests
/// deeper than [`MAX_DEPTH`] (the parser recurses per level).
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
        flags_span_offset: lit.span.end - flags.len() as u32,
    };
    LiteralParser::new(allocator, pattern, Some(flags), options)
        .parse()
        .ok()
}

/// Calls `f` for every disjunction of the pattern, with `true` for the top-level one.
pub(super) fn for_each_disjunction<'a>(
    pattern: &Pattern<'a>,
    f: &mut dyn FnMut(&Disjunction<'a>, bool),
) {
    walk_disjunction(&pattern.body, true, 0, f, &mut |_| {});
}

/// Calls `f` for every character class of the pattern, nested classes included.
pub(super) fn for_each_class<'a>(pattern: &Pattern<'a>, f: &mut dyn FnMut(&CharacterClass<'a>)) {
    walk_disjunction(&pattern.body, true, 0, &mut |_, _| {}, f);
}

fn walk_disjunction<'a>(
    disjunction: &Disjunction<'a>,
    top: bool,
    depth: usize,
    on_disjunction: &mut dyn FnMut(&Disjunction<'a>, bool),
    on_class: &mut dyn FnMut(&CharacterClass<'a>),
) {
    if depth > MAX_DEPTH {
        return;
    }
    on_disjunction(disjunction, top);
    for alternative in &disjunction.body {
        for term in &alternative.body {
            walk_term(term, depth + 1, on_disjunction, on_class);
        }
    }
}

fn walk_term<'a>(
    term: &Term<'a>,
    depth: usize,
    on_disjunction: &mut dyn FnMut(&Disjunction<'a>, bool),
    on_class: &mut dyn FnMut(&CharacterClass<'a>),
) {
    if depth > MAX_DEPTH {
        return;
    }
    match term {
        Term::LookAroundAssertion(t) => {
            walk_disjunction(&t.body, false, depth, on_disjunction, on_class);
        }
        Term::CapturingGroup(t) => {
            walk_disjunction(&t.body, false, depth, on_disjunction, on_class)
        }
        Term::IgnoreGroup(t) => walk_disjunction(&t.body, false, depth, on_disjunction, on_class),
        Term::Quantifier(t) => walk_term(&t.body, depth + 1, on_disjunction, on_class),
        Term::CharacterClass(c) => walk_class(c, depth + 1, on_class),
        _ => {}
    }
}

fn walk_class<'a>(
    class: &CharacterClass<'a>,
    depth: usize,
    on_class: &mut dyn FnMut(&CharacterClass<'a>),
) {
    if depth > MAX_DEPTH {
        return;
    }
    on_class(class);
    for item in &class.body {
        if let CharacterClassContents::NestedCharacterClass(nested) = item {
            walk_class(nested, depth + 1, on_class);
        }
    }
}
