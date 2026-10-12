//! `js/redundant-regex-syntax`

use crate::prelude::*;
use oxc_regular_expression::ast::{
    Character, CharacterClass, CharacterClassContents, CharacterClassContentsKind,
    CharacterClassEscapeKind, Disjunction, Term,
};
use oxc_regular_expression::{LiteralParser, Options};

pub struct RedundantRegexSyntax;

pub static REDUNDANT_REGEX_SYNTAX: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["regex", "redundant", "readability"],
    remediation_minutes: 2,
    description: r#"Flags parts of regular expression literals that say nothing or say it the long way:
quantifiers of exactly one (`a{1}`, `a{1,1}`), long forms of the short quantifiers (`{0,}` for `*`,
`{1,}` for `+`, `{0,1}` for `?`), character classes holding a member twice or a member another
member already covers (`[aa]`, `[a-zx]`, `[\d0-9]`, `[\w_]`), classes of a single letter, digit or
underscore (`[a]`, `[_]`), empty non-capturing groups (`(?:)`, `a(?:|)b`), alternations whose
branches are all single characters (`a|b|c`, `\s|,`), group names that nothing reads, and runs of
literal spaces (`/a   b/`).

**Why:** a regular expression is read symbol by symbol. Redundant parts make the reader wonder
what they add, and a class with a repeated or covered member often means that the author meant a
different member (`[a-zA-z]` for `[a-zA-Z]`). An empty non-capturing group matches only the
empty string and does nothing: it is usually a leftover of an edit, or content that was never
filled in. An alternation of single characters makes the reader check every branch for a longer
one, where a class states in one step that one character is matched. A group name that nothing
reads suggests code that looks the group up, and there is none. A run of spaces cannot be counted
at a glance.

**Fix:** drop the redundant part (or fill in the empty group), use the short form, or state the
count (`/a {3}b/`).

```js
const id = /[0-9]{1}[a-z_a]+/;      // flagged: {1}, and 'a' is inside a-z
const id = /[0-9][a-z_]+/;          // compliant

const time = /\d{1,}:\d{0,1}/;      // flagged: {1,} is +, {0,1} is ?
const time = /\d+:\d?/;             // compliant

const list = /\s*(,|;)\s*/;         // flagged: one of two characters
const list = /\s*([,;])\s*/;        // compliant

line.split(/(?<sep>[,;])/);         // flagged: nothing reads the name 'sep'
line.split(/([,;])/);               // compliant
```

Only literals (`/…/`) are checked. Not reported: escapes (see `js/no-useless-escape`), a
single-character class used to quote a special character (`[.]`, `[*]`) or a space (`[ ]`),
members that only coincide under the `i` flag, classes using set operations (`v` flag), and
classes holding a word without any `|` (`[\spunctuation]`, usually a placeholder that code replaces
in the pattern's source), empty capturing groups (`()`, `(?<name>)`: they still take a group
number, which code may rely on to pad the numbering, and with the `d` flag they record a
position), a pattern that is nothing but an empty group (`/(?:)/`, the usual way to write an
empty regular expression), an empty non-capturing group followed by a digit (`\1(?:)0`, which
keeps a back-reference from swallowing the digit), alternations with a repeated branch (see
`js/no-dupe-disjunctions`) or with a branch that is `.`, a property escape (`\p{L}`) or longer
than one character, and named groups of a literal whose matches code may read: group names are
checked only for a literal passed straight to `test`, `search`, `split`, or `replace` /
`replaceAll` with a replacement string that has no `$<`, and a `\k<name>` in the pattern counts
as a use.
"#,
    ..RuleMeta::new(
        "js/redundant-regex-syntax",
        "js",
        "Regular expressions should not contain redundant syntax",
    )
};

impl JsRule for RedundantRegexSyntax {
    fn meta(&self) -> &'static RuleMeta {
        &REDUNDANT_REGEX_SYNTAX
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::RegExpLiteral]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::RegExpLiteral(lit) = node.kind() else {
            return;
        };
        let pattern = lit.regex.pattern.text.as_str();
        // Cheap text test: every shape needs `{`, `[`, `|`, two spaces, `:)` (an empty
        // non-capturing group) or `(?<` (a named group).
        let named = pattern.contains("(?<");
        if !pattern.contains(['{', '[', '|'])
            && !pattern.contains("  ")
            && !pattern.contains(":)")
            && !named
        {
            return;
        }
        // The whole pattern an empty group: the usual spelling of an empty regular expression.
        if pattern == "(?:)" {
            return;
        }
        if !crate::helpers::regex_within_limits(pattern, crate::helpers::REGEX_MAX_NESTING) {
            return;
        }
        let raw = ctx.text(lit.span);
        let flags = raw.rsplit('/').next().unwrap_or("");
        let allocator = oxc_allocator::Allocator::default();
        let Ok(parsed) = LiteralParser::new(
            &allocator,
            pattern,
            Some(flags),
            Options {
                pattern_span_offset: lit.span.start + 1,
                flags_span_offset: 0,
            },
        )
        .parse() else {
            return;
        };
        let mut findings: Vec<(Span, String)> = Vec::new();
        let ignore_case = lit.regex.flags.contains(RegExpFlags::I);
        visit_disjunction(ctx, &parsed.body, ignore_case, &mut findings);
        if let Some(replacement) = named.then(|| name_readers(ctx, node)).flatten() {
            let mut groups = Groups::default();
            collect_names(&parsed.body, 0, &mut groups);
            for (name, span) in groups.defined {
                if !groups.referenced.contains(&name)
                    && !replacement.contains(&format!("$<{name}>"))
                {
                    findings.push((
                        span,
                        format!("Remove the name '{name}' from this group: nothing reads it"),
                    ));
                }
            }
        }
        for (span, message) in findings {
            ctx.emit(span, message);
        }
    }
}

/// When the literal is passed straight to a call that reads its groups by name only through a
/// replacement string, that string (empty when nothing can read them): the object of `test`, the
/// pattern of `search` or `split`, or the pattern of `replace` / `replaceAll` with a replacement
/// string. `None` for every other use, where code may read the groups.
fn name_readers<'a>(ctx: &Ctx<'a>, literal: &AstNode<'a>) -> Option<&'a str> {
    let parent = ctx.outer_parent(literal)?;
    let span = literal.span();
    match parent.kind() {
        // `/…/.test(s)`
        AstKind::StaticMemberExpression(member) => {
            if member.property.name != "test" || member.object.get_inner_expression().span() != span
            {
                return None;
            }
            let AstKind::CallExpression(call) = ctx.outer_parent(parent)?.kind() else {
                return None;
            };
            (call.callee.get_inner_expression().span() == member.span).then_some("")
        }
        // `s.split(/…/)`, `s.replace(/…/, '…')`
        AstKind::CallExpression(call) => {
            let first = call.arguments.first()?.as_expression()?;
            if first.get_inner_expression().span() != span {
                return None;
            }
            let member = call.callee.get_inner_expression().as_member_expression()?;
            match member.static_property_name()? {
                "search" | "split" => Some(""),
                "replace" | "replaceAll" => call
                    .arguments
                    .get(1)
                    .and_then(Argument::as_expression)
                    .and_then(|r| helpers::string_value(helpers::unwrap_expr(r))),
                _ => None,
            }
        }
        _ => None,
    }
}

/// The named groups of a pattern (name and the span of `(?<name>`) and the names its `\k<…>`
/// back-references use.
#[derive(Default)]
struct Groups {
    defined: Vec<(String, Span)>,
    referenced: Vec<String>,
}

fn collect_names(d: &Disjunction<'_>, depth: usize, out: &mut Groups) {
    if depth > 64 {
        return;
    }
    for alternative in &d.body {
        for term in &alternative.body {
            collect_term_names(term, depth + 1, out);
        }
    }
}

fn collect_term_names(term: &Term<'_>, depth: usize, out: &mut Groups) {
    if depth > 64 {
        return;
    }
    match term {
        Term::CapturingGroup(g) => {
            if let Some(name) = &g.name {
                // `(?<` + name + `>`
                let end = g.span.start + 4 + name.as_str().len() as u32;
                out.defined
                    .push((name.as_str().to_string(), Span::new(g.span.start, end)));
            }
            collect_names(&g.body, depth, out);
        }
        Term::IgnoreGroup(g) => collect_names(&g.body, depth, out),
        Term::LookAroundAssertion(l) => collect_names(&l.body, depth, out),
        Term::Quantifier(q) => collect_term_names(&q.body, depth + 1, out),
        Term::NamedReference(r) => out.referenced.push(r.name.as_str().to_string()),
        _ => {}
    }
}

/// An alternation whose branches are each one character or one class escape (`a|b`, `\s|,`),
/// with no branch repeated.
fn is_single_character_alternation(ctx: &Ctx<'_>, d: &Disjunction<'_>, ignore_case: bool) -> bool {
    if d.body.len() < 2 {
        return false;
    }
    let mut seen: Vec<&str> = Vec::with_capacity(d.body.len());
    for alternative in &d.body {
        let [term] = alternative.body.as_slice() else {
            return false;
        };
        let span = match term {
            Term::Character(c) if !is_sequence_part(c.value) => c.span,
            Term::CharacterClassEscape(e) => e.span,
            _ => return false,
        };
        let text = ctx.text(span);
        // A repeated branch is `js/no-dupe-disjunctions`' case.
        if seen
            .iter()
            .any(|s| *s == text || (ignore_case && s.eq_ignore_ascii_case(text)))
        {
            return false;
        }
        seen.push(text);
    }
    true
}

fn visit_disjunction(
    ctx: &Ctx<'_>,
    d: &Disjunction<'_>,
    ignore_case: bool,
    out: &mut Vec<(Span, String)>,
) {
    if is_single_character_alternation(ctx, d, ignore_case) {
        out.push((
            d.span,
            format!(
                "Replace the alternation '{}' with a character class: each branch is a single character",
                ctx.text(d.span)
            ),
        ));
    }
    for alternative in &d.body {
        // Runs of literal spaces.
        let mut run: Vec<Span> = Vec::new();
        for term in &alternative.body {
            match term {
                Term::Character(c) if c.value == u32::from(b' ') && ctx.text(c.span) == " " => {
                    run.push(c.span);
                    continue;
                }
                _ => flush_spaces(&mut run, out),
            }
            visit_term(ctx, term, ignore_case, out);
        }
        flush_spaces(&mut run, out);
    }
}

fn flush_spaces(run: &mut Vec<Span>, out: &mut Vec<(Span, String)>) {
    if run.len() >= 2 {
        if let (Some(first), Some(last)) = (run.first(), run.last()) {
            out.push((
                Span::new(first.start, last.end),
                format!(
                    "Write ' {{{}}}' instead of {} spaces in a row",
                    run.len(),
                    run.len()
                ),
            ));
        }
    }
    run.clear();
}

fn visit_term(ctx: &Ctx<'_>, term: &Term<'_>, ignore_case: bool, out: &mut Vec<(Span, String)>) {
    match term {
        Term::Quantifier(q) => {
            let body_end = q.body.span().end;
            let suffix = ctx.text(Span::new(body_end, q.span.end));
            let suffix = suffix
                .strip_suffix('?')
                .filter(|_| !q.greedy)
                .unwrap_or(suffix);
            let replacement = match (q.min, q.max) {
                (1, Some(1)) if suffix.starts_with('{') => Some(None),
                (0, None) if suffix.starts_with('{') => Some(Some("*")),
                (1, None) if suffix.starts_with('{') => Some(Some("+")),
                (0, Some(1)) if suffix.starts_with('{') => Some(Some("?")),
                _ => None,
            };
            if let Some(replacement) = replacement {
                let span = Span::new(body_end, body_end + suffix.len() as u32);
                let message = match replacement {
                    None => format!("Remove '{suffix}': it repeats exactly once"),
                    Some(short) => format!("Write '{short}' instead of '{suffix}'"),
                };
                out.push((span, message));
            }
            visit_term(ctx, &q.body, ignore_case, out);
        }
        Term::CharacterClass(class) => visit_class(ctx, class, out),
        Term::CapturingGroup(g) => visit_disjunction(ctx, &g.body, ignore_case, out),
        Term::IgnoreGroup(g) => {
            if is_empty(&g.body) {
                // `\1(?:)0`: the group keeps the back-reference from reading the digit.
                let next = ctx.text(Span::new(g.span.end, g.span.end + 1));
                if !next.starts_with(|c: char| c.is_ascii_digit()) {
                    out.push((
                        g.span,
                        "Remove this empty group or fill it in: it matches only the empty string"
                            .to_string(),
                    ));
                }
            } else {
                visit_disjunction(ctx, &g.body, ignore_case, out);
            }
        }
        Term::LookAroundAssertion(l) => visit_disjunction(ctx, &l.body, ignore_case, out),
        _ => {}
    }
}

/// What a class member matches, for comparison: a code point range or an escape.
#[derive(Clone, Copy, PartialEq)]
enum Member {
    Range(u32, u32),
    Escape(CharacterClassEscapeKind),
}

fn visit_class(ctx: &Ctx<'_>, class: &CharacterClass<'_>, out: &mut Vec<(Span, String)>) {
    if class.kind != CharacterClassContentsKind::Union || class.strings {
        return;
    }
    // A single letter, digit or underscore: the class adds nothing.
    if let [CharacterClassContents::Character(c)] = class.body.as_slice() {
        if !class.negative && is_plain_char(c) {
            out.push((
                class.span,
                format!(
                    "Write '{}' instead of the one-character class",
                    ctx.text(c.span)
                ),
            ));
        }
        return;
    }
    let members: Vec<(Member, Span)> = class
        .body
        .iter()
        .filter_map(|m| match m {
            CharacterClassContents::Character(c) => Some((Member::Range(c.value, c.value), c.span)),
            CharacterClassContents::CharacterClassRange(r) => {
                Some((Member::Range(r.min.value, r.max.value), r.span))
            }
            CharacterClassContents::CharacterClassEscape(e) => {
                Some((Member::Escape(e.kind), e.span))
            }
            _ => None,
        })
        .collect();
    // A word inside a class without `|` (`[\spunctuation]`) is usually a placeholder that code
    // replaces in the pattern's source before use.
    let text = ctx.text(class.span);
    if !text.contains('|') && has_word(text) {
        return;
    }
    // Emoji and combining sequences split into code units: a different problem (the class
    // does not hold what it seems to), left alone here.
    if members.iter().any(
        |(m, _)| matches!(m, Member::Range(a, b) if is_sequence_part(*a) || is_sequence_part(*b)),
    ) {
        return;
    }
    // One finding per class: the first redundant member, and how many more there are.
    let mut redundant: Vec<(Span, Span)> = Vec::new();
    for (i, (member, span)) in members.iter().enumerate() {
        let covering = members.iter().enumerate().find(|(j, (other, _))| {
            *j != i && covers(*other, *member) && (*other != *member || *j < i)
        });
        if let Some((_, (_, other_span))) = covering {
            redundant.push((*span, *other_span));
        }
    }
    let Some(&(span, other_span)) = redundant.first() else {
        return;
    };
    let text = ctx.text(span);
    let other = ctx.text(other_span);
    let mut message = if text == other {
        format!("Remove the repeated '{text}' from this character class")
    } else {
        format!("Remove '{text}' from this character class: '{other}' already matches it")
    };
    if redundant.len() > 1 {
        message.push_str(&format!(" ({} more redundant members", redundant.len() - 1));
        if ctx.text(class.span).contains('|') {
            message.push_str("; '|' does not separate alternatives inside a class");
        }
        message.push(')');
    }
    out.push((span, message));
}

/// A group whose alternatives are all empty: `(?:)`, `(?:|)`.
fn is_empty(d: &Disjunction<'_>) -> bool {
    d.body.iter().all(|alternative| alternative.body.is_empty())
}

/// Four ASCII letters in a row.
fn has_word(text: &str) -> bool {
    let mut run = 0;
    for b in text.bytes() {
        if b.is_ascii_alphabetic() {
            run += 1;
            if run >= 4 {
                return true;
            }
        } else {
            run = 0;
        }
    }
    false
}

/// Surrogate halves, joiners, variation selectors and combining marks: parts of a sequence that
/// a class without the `u`/`v` flag splits apart.
fn is_sequence_part(v: u32) -> bool {
    matches!(v, 0x0300..=0x036F | 0x200C | 0x200D | 0xD800..=0xDFFF | 0xFE00..=0xFE0F)
}

/// Does `outer` match everything `inner` matches?
fn covers(outer: Member, inner: Member) -> bool {
    use CharacterClassEscapeKind as K;
    match (outer, inner) {
        (Member::Range(a, b), Member::Range(c, d)) => a <= c && d <= b,
        (Member::Escape(a), Member::Escape(b)) => a == b || (a == K::W && b == K::D),
        (Member::Escape(K::D), Member::Range(c, d)) => is_within(c, d, b'0', b'9'),
        (Member::Escape(K::W), Member::Range(c, d)) => {
            is_within(c, d, b'0', b'9')
                || is_within(c, d, b'a', b'z')
                || is_within(c, d, b'A', b'Z')
                || (c == d && c == u32::from(b'_'))
        }
        _ => false,
    }
}

fn is_within(c: u32, d: u32, lo: u8, hi: u8) -> bool {
    u32::from(lo) <= c && d <= u32::from(hi)
}

/// A letter, digit or underscore written as itself.
fn is_plain_char(c: &Character) -> bool {
    char::from_u32(c.value).is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        && c.span.end - c.span.start == 1
}
