//! `js/useless-regex-quantifier`

use crate::prelude::*;
use oxc_regular_expression::ast::{Disjunction, Term};
use oxc_regular_expression::{LiteralParser, Options};

pub struct UselessRegexQuantifier;

pub static USELESS_REGEX_QUANTIFIER: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["regex", "suspicious"],
    remediation_minutes: 2,
    description: r#"Flags quantifiers in regular expression literals that make a part of the pattern do nothing:

- a count of zero (`a{0}`, `(?:ab){0,0}`): the part is repeated zero times, so it never matches
  anything and the pattern behaves as if it were not written;
- an optional quantifier (`?`, `*`, `{0,n}`) on an assertion, a part that matches a position
  rather than characters (`(?=\d)?`, `(?:^)*`, `(?:\b|$)?`): the engine can always take the
  zero-repetition path, so the assertion never constrains the match.

**Why:** in both cases the pattern accepts inputs the author meant to reject (or rejects nothing
the part was written for), and nothing fails loudly: the check just silently does not happen. A
zero count is usually a typo for another count (`{0,3}`, `{10}`), and an optional assertion is
usually a quantifier meant for the element next to it.

**Fix:** correct the count or remove the dead part; drop the quantifier from the assertion (or
move it to the element it was meant for).

```js
const code = /^[A-Z]{0}\d+$/;     // flagged: [A-Z] never matches
const code = /^[A-Z]{0,2}\d+$/;   // compliant

const word = /(?=\w)?\w+/;        // flagged: the lookahead is optional
const word = /(?=\w)\w+/;         // compliant
```

Only regular expression literals are checked. Quantifiers of one or more on an assertion
(`(?:\b)+`) do no harm and are not reported, nor are quantifiers on empty groups (see
`js/redundant-regex-syntax`).
"#,
    ..RuleMeta::new(
        "js/useless-regex-quantifier",
        "js",
        "Regex quantifiers should not make a part of the pattern ineffective",
    )
};

impl JsRule for UselessRegexQuantifier {
    fn meta(&self) -> &'static RuleMeta {
        &USELESS_REGEX_QUANTIFIER
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::RegExpLiteral]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::RegExpLiteral(lit) = node.kind() else {
            return;
        };
        let pattern = lit.regex.pattern.text.as_str();
        // Cheap text test: a zero count needs `{0`; a quantified assertion is always a group or a
        // lookaround, so a `)` followed by a quantifier.
        if !pattern.contains("{0") && !has_quantified_group_end(pattern) {
            return;
        }
        if !crate::helpers::regex_within_limits(pattern, crate::helpers::REGEX_MAX_NESTING) {
            return;
        }
        let raw = ctx.text(lit.span);
        let flags = match raw.rfind('/') {
            Some(i) => raw.get(i + 1..).unwrap_or(""),
            None => "",
        };
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
        let mut found: Vec<(Span, &'static str)> = Vec::new();
        visit_disjunction(&parsed.body, 0, &mut found);
        for (span, message) in found {
            ctx.emit(span, message);
        }
    }
}

const ZERO_COUNT: &str =
    "Fix the count or remove this part: repeated zero times, it never matches anything";
const OPTIONAL_ASSERTION: &str = "Remove this quantifier: it makes the assertion optional, so the assertion never constrains the match";

fn visit_disjunction(d: &Disjunction<'_>, depth: usize, out: &mut Vec<(Span, &'static str)>) {
    if depth > 64 {
        return;
    }
    for alternative in &d.body {
        for term in &alternative.body {
            visit_term(term, depth + 1, out);
        }
    }
}

fn visit_term(term: &Term<'_>, depth: usize, out: &mut Vec<(Span, &'static str)>) {
    if depth > 64 {
        return;
    }
    match term {
        Term::Quantifier(q) => {
            let quantifier = Span::new(q.body.span().end, q.span.end);
            if q.max == Some(0) {
                out.push((quantifier, ZERO_COUNT));
                // Whatever the dead part contains does not matter any more.
                return;
            }
            if q.min == 0 && zero_width(&q.body, depth) == Some(true) {
                out.push((quantifier, OPTIONAL_ASSERTION));
            }
            visit_term(&q.body, depth + 1, out);
        }
        Term::CapturingGroup(g) => visit_disjunction(&g.body, depth, out),
        Term::IgnoreGroup(g) => visit_disjunction(&g.body, depth, out),
        Term::LookAroundAssertion(l) => visit_disjunction(&l.body, depth, out),
        _ => {}
    }
}

/// Does the term only ever match an empty string (a position)? `None` when it can consume
/// characters, else whether it holds an assertion (empty groups hold none: they are reported by
/// `js/redundant-regex-syntax`).
fn zero_width(term: &Term<'_>, depth: usize) -> Option<bool> {
    if depth > 64 {
        return None;
    }
    let body = match term {
        Term::BoundaryAssertion(_) | Term::LookAroundAssertion(_) => return Some(true),
        Term::CapturingGroup(g) => &g.body,
        Term::IgnoreGroup(g) => &g.body,
        _ => return None,
    };
    let mut assertion = false;
    for alternative in &body.body {
        for t in &alternative.body {
            assertion |= zero_width(t, depth + 1)?;
        }
    }
    Some(assertion)
}

/// A `)` followed by a quantifier: the only place an assertion can be quantified.
fn has_quantified_group_end(pattern: &str) -> bool {
    pattern
        .as_bytes()
        .windows(2)
        .any(|w| w[0] == b')' && matches!(w[1], b'?' | b'*' | b'{'))
}
