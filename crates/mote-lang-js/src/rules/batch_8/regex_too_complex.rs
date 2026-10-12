//! `js/regex-too-complex`

use crate::prelude::*;
use oxc_regular_expression::ast::{Disjunction, Term};
use oxc_regular_expression::{LiteralParser, Options};

pub struct RegexTooComplex;

pub static REGEX_TOO_COMPLEX: RuleMeta = RuleMeta {
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["regex", "complexity", "readability"],
    remediation_minutes: 20,
    params: &[ParamSpec {
        name: "max",
        default: ParamDefault::Int(20),
        description: "Maximum complexity score of a regular expression literal.",
    }],
    description: r#"Flags regular expression literals whose structure scores above `max` (default 20).

**Why:** a regular expression packs a whole matcher into one line with no names and no comments.
Past a certain size nobody can check by reading whether it accepts what it should, changes become
guesswork, and nested repetitions are where catastrophic backtracking hides.

**Fix:** split the pattern into named parts (`const date = /…/.source`), check the simple parts with
string methods, or use a small parser for structured input; at the least, document the parts.

The score adds, for each element, one point plus one per level of group nesting it sits in:
each alternative after the first (`|`; a list of plain words such as `(get|put|post)` counts as a
single alternative), each repetition of a group with variable length (`(…)*`, `(…)?`), and each
lookaround; repetitions of a single character, class or escape (`\s+`, `[0-9]*`), character
classes and back-references add one point each. Fixed repetitions (`\d{4}`), plain characters, escapes and anchors are free.

```js
// flagged
const semver = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\+([0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$/;

// compliant: the parts are named
const num = /0|[1-9]\d*/.source;
const ident = /[0-9a-zA-Z-]+/.source;
const semver = new RegExp(
  `^(${num})\\.(${num})\\.(${num})(?:-(${ident}(?:\\.${ident})*))?(?:\\+(${ident}(?:\\.${ident})*))?$`,
);
```

Only literals (`/…/`) are checked.
"#,
    ..RuleMeta::new(
        "js/regex-too-complex",
        "js",
        "Regular expressions should not be too complex",
    )
};

impl JsRule for RegexTooComplex {
    fn meta(&self) -> &'static RuleMeta {
        &REGEX_TOO_COMPLEX
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::RegExpLiteral]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::RegExpLiteral(lit) = node.kind() else {
            return;
        };
        let pattern = lit.regex.pattern.text.as_str();
        // Cheap bound first: every scoring element needs at least one character.
        let max = ctx.param_i64("max").max(0) as u32;
        if (pattern.len() as u32) <= max {
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
        let score = disjunction_score(&parsed.body, 0);
        if score <= max {
            return;
        }
        let shown = Span::new(lit.span.start, lit.span.end.min(lit.span.start + 60));
        ctx.emit(
            shown,
            format!("Simplify this regular expression: its complexity is {score} (at most {max} allowed)"),
        )
        .value(f64::from(score));
    }
}

fn disjunction_score(d: &Disjunction<'_>, nesting: u32) -> u32 {
    // A list of plain words (`(get|put|post)`) reads as one choice: literal alternatives count
    // once together, alternatives with structure count one each.
    let plain = d
        .body
        .iter()
        .filter(|a| a.body.iter().all(|t| matches!(t, Term::Character(_))))
        .count() as u32;
    let structured = d.body.len() as u32 - plain;
    let choices = structured + u32::from(plain > 0);
    let mut score = choices.saturating_sub(1) * (1 + nesting);
    for alternative in &d.body {
        for term in &alternative.body {
            score = score.saturating_add(term_score(term, nesting));
        }
    }
    score
}

fn term_score(term: &Term<'_>, nesting: u32) -> u32 {
    match term {
        Term::Quantifier(q) => {
            // Repeating a group costs more the deeper it sits; repeating a single atom (`\s+`,
            // `[0-9]*`) reads the same at any depth.
            let own = if q.max == Some(q.min) {
                0
            } else if matches!(
                q.body,
                Term::CapturingGroup(_) | Term::IgnoreGroup(_) | Term::LookAroundAssertion(_)
            ) {
                1 + nesting
            } else {
                1
            };
            own + term_score(&q.body, nesting)
        }
        Term::LookAroundAssertion(l) => 1 + nesting + disjunction_score(&l.body, nesting + 1),
        Term::CapturingGroup(g) => disjunction_score(&g.body, nesting + 1),
        Term::IgnoreGroup(g) => disjunction_score(&g.body, nesting + 1),
        Term::CharacterClass(_) | Term::IndexedReference(_) | Term::NamedReference(_) => 1,
        _ => 0,
    }
}
