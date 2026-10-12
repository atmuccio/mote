//! `js/no-dupe-disjunctions`

use super::regex;
use crate::prelude::*;
use oxc_regular_expression::ast::{Alternative, Term};

pub struct NoDupeDisjunctions;

pub static NO_DUPE_DISJUNCTIONS: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["regex", "suspicious"],
    remediation_minutes: 5,
    description: r#"Flags an alternative of a regular expression that can never be the one that matches: it repeats
an earlier alternative of the same group (`/(png|jpg|png)/`), or, at the top level of the
pattern, an earlier alternative already matches its beginning (`/\r|\r\n/`).

**Why:** alternatives are tried from left to right, and the first one that matches wins. A copy
of an earlier alternative is dead weight — usually a typo for the alternative that was meant
(`jpeg`?). At the top level, where nothing follows the alternatives, a shorter alternative that is
a prefix of a later one always wins, so the longer one is never used: `/\r|\r\n/` splits Windows
line ends into two breaks (and an empty line between them), and `/Mon|Monday/` replaces only
`Mon`.

**Fix:** remove the duplicate or correct it; put longer alternatives before their prefixes
(`/\r\n|\r/`).

```js
/\.(png|jpg|png)$/     // flagged: 'png' twice
text.split(/\r|\r\n/)  // flagged: '\r\n' is never matched
/\.(png|jpe?g)$/       // compliant
text.split(/\r\n|\r/)  // compliant
```

Only regular expression literals are checked. Inside a group, a shorter alternative does not hide
a longer one (backtracking can still choose it when what follows the group fails), so that case is
not reported; the top-level case is only reported when the literal is passed directly to `split`,
`replace`, `replaceAll`, `match` or `matchAll`, or `exec` is called on it — a literal stored for
later may be spliced into a bigger pattern, and `test` or `search` give the same answer either way.
"#,
    ..RuleMeta::new(
        "js/no-dupe-disjunctions",
        "js",
        "Regular expression alternatives should not be repeated or hidden by earlier ones",
    )
};

/// Beyond this many alternatives the prefix comparison (quadratic) is skipped.
const MAX_PREFIX_ALTERNATIVES: usize = 64;

impl JsRule for NoDupeDisjunctions {
    fn meta(&self) -> &'static RuleMeta {
        &NO_DUPE_DISJUNCTIONS
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::RegExpLiteral]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::RegExpLiteral(lit) = node.kind() else {
            return;
        };
        if !lit.regex.pattern.text.contains('|') {
            return;
        }
        let allocator = oxc_allocator::Allocator::default();
        let Some(pattern) = regex::parse(&allocator, ctx, lit) else {
            return;
        };
        // A pattern can be a fragment spliced into a bigger one (`re.source`), where more follows
        // its alternatives; the prefix check needs the literal to be used as it is, for its
        // matched text.
        let check_prefixes = matches_directly(ctx, node);
        let mut found: Vec<(Span, String)> = Vec::new();
        regex::for_each_disjunction(&pattern, &mut |disjunction, top| {
            let top = top && check_prefixes;
            let alternatives = &disjunction.body;
            if alternatives.len() < 2 {
                return;
            }
            let mut seen: FxHashSet<&str> = FxHashSet::default();
            for (j, alternative) in alternatives.iter().enumerate() {
                let text = ctx.text(alternative.span);
                if text.is_empty() {
                    continue;
                }
                if !seen.insert(text) {
                    found.push((
                        alternative.span,
                        format!(
                            "Remove or fix this alternative: '{}' already appears earlier in the same group, so it can never match",
                            excerpt(text)
                        ),
                    ));
                    continue;
                }
                if !top || alternatives.len() > MAX_PREFIX_ALTERNATIVES {
                    continue;
                }
                if let Some(earlier) = alternatives[..j]
                    .iter()
                    .find(|a| shadows(ctx, a, alternative))
                {
                    found.push((
                        alternative.span,
                        format!(
                            "This alternative can never match: the earlier alternative '{}' already matches its beginning and wins; put the longer alternative first",
                            excerpt(ctx.text(earlier.span))
                        ),
                    ));
                }
            }
        });
        for (span, message) in found {
            ctx.emit(span, message);
        }
    }
}

/// Is the literal used right where it is written, for the text it matches: the first argument of
/// `split`, `replace`, `replaceAll`, `match` or `matchAll`, or the receiver of `exec`?
/// (`test` and `search` only care whether and where a match starts, which the order of the
/// alternatives does not change.)
fn matches_directly(ctx: &Ctx<'_>, literal: &AstNode<'_>) -> bool {
    let at = literal.span();
    let Some(parent) = ctx.outer_parent(literal) else {
        return false;
    };
    match parent.kind() {
        AstKind::CallExpression(call) => {
            call.arguments.first().is_some_and(|a| a.span() == at)
                && matches!(
                    helpers::unwrap_expr(&call.callee),
                    Expression::StaticMemberExpression(m)
                        if matches!(m.property.name.as_str(), "split" | "replace" | "replaceAll" | "match" | "matchAll")
                )
        }
        AstKind::StaticMemberExpression(m) => {
            m.property.name == "exec"
                && matches!(
                    ctx.outer_parent(parent).map(AstNode::kind),
                    Some(AstKind::CallExpression(_))
                )
        }
        _ => false,
    }
}

/// Does `earlier` match wherever `later` does, at the same position? True when `earlier` is a
/// sequence of single-character matchers and assertions (no quantifiers, groups or references,
/// so its match is fixed) and `later` starts with exactly the same terms.
fn shadows(ctx: &Ctx<'_>, earlier: &Alternative<'_>, later: &Alternative<'_>) -> bool {
    let (a, b) = (&earlier.body, &later.body);
    if a.is_empty() || a.len() >= b.len() {
        return false;
    }
    a.iter().zip(b.iter()).all(|(x, y)| {
        let text = ctx.text(term_span(x));
        matches!(
            x,
            Term::Character(_)
                | Term::CharacterClass(_)
                | Term::CharacterClassEscape(_)
                | Term::UnicodePropertyEscape(_)
                | Term::Dot(_)
                | Term::BoundaryAssertion(_)
        ) && !text.is_empty()
            && text == ctx.text(term_span(y))
    })
}

fn term_span(term: &Term<'_>) -> Span {
    match term {
        Term::BoundaryAssertion(t) => t.span,
        Term::LookAroundAssertion(t) => t.span,
        Term::Quantifier(t) => t.span,
        Term::Character(t) => t.span,
        Term::Dot(t) => t.span,
        Term::CharacterClassEscape(t) => t.span,
        Term::UnicodePropertyEscape(t) => t.span,
        Term::CharacterClass(t) => t.span,
        Term::CapturingGroup(t) => t.span,
        Term::IgnoreGroup(t) => t.span,
        Term::IndexedReference(t) => t.span,
        Term::NamedReference(t) => t.span,
    }
}

/// At most 30 characters of a pattern piece, for messages.
fn excerpt(text: &str) -> String {
    let mut chars = text.chars();
    let short: String = chars.by_ref().take(30).collect();
    if chars.next().is_some() {
        format!("{short}…")
    } else {
        short
    }
}
