//! `js/regex-lazy-quantifier-at-end`

use super::regex::{self, MAX_DEPTH};
use crate::prelude::*;
use oxc_regular_expression::ast::{Disjunction, Quantifier, Term};

pub struct RegexLazyQuantifierAtEnd;

pub static REGEX_LAZY_QUANTIFIER_AT_END: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["regex", "suspicious"],
    remediation_minutes: 5,
    description: r#"Flags a lazy quantifier (`*?`, `+?`, `??`, `{n,m}?`) at the very end of a regular expression
literal that is searched for in a string: the pattern of `replace`, `replaceAll`, `split`,
`match`, `matchAll` or `search`, or a literal whose `exec` or `test` is called directly
(`/<!--.*?/.test(html)`).

**Why:** a lazy quantifier repeats as few times as it can, and takes one more repetition only when
the rest of the pattern fails to match without it. At the end of the pattern nothing follows, so
nothing ever asks for more: `.*?` matches the empty string, `\w+?` exactly one character, `{2,5}?`
exactly two. The pattern reads as if it captured "the rest", but it never does. Usually the part
that should end the match (`-->`, a closing quote, `$`) was forgotten.

**Fix:** write what ends the match after the quantifier, or make the quantifier greedy if the
longest run was meant.

```js
html.replace(/<!--.*?/g, '');        // flagged: removes only "<!--"
html.replace(/<!--.*?-->/g, '');     // compliant

const [, key] = /key=(\w+?)/.exec(line);   // flagged: captures one character
const [, key] = /key=(\w+)/.exec(line);    // compliant
```

Not reported: a lazy quantifier followed by an anchor or assertion (`.*?$`, `\w+?\b`, a
lookahead), which makes it extend; a fixed count (`{2}?`); lazy quantifiers inside lookarounds or
inside a repeated group; and regular expressions stored in a variable or built with `new RegExp`,
whose use is not visible where they are written.
"#,
    ..RuleMeta::new(
        "js/regex-lazy-quantifier-at-end",
        "js",
        "Lazy quantifiers should not end a searched regular expression",
    )
};

/// Methods that search a string for the pattern: the literal is their first argument, or (for
/// `exec` and `test`) the object they are called on.
const CALL_NAMES: &[&str] = &[
    "replace",
    "replaceAll",
    "split",
    "match",
    "matchAll",
    "search",
    "exec",
    "test",
];

impl JsRule for RegexLazyQuantifierAtEnd {
    fn meta(&self) -> &'static RuleMeta {
        &REGEX_LAZY_QUANTIFIER_AT_END
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CallExpression]
    }

    fn callee_names(&self) -> &'static [&'static str] {
        CALL_NAMES
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::CallExpression(call) = node.kind() else {
            return;
        };
        let Some(lit) = searched_literal(call) else {
            return;
        };
        let pattern = lit.regex.pattern.text.as_str();
        // Cheap text test: a lazy quantifier ends in `?` after another quantifier.
        if !(pattern.contains("*?")
            || pattern.contains("+?")
            || pattern.contains("??")
            || pattern.contains("}?"))
        {
            return;
        }
        let allocator = oxc_allocator::Allocator::default();
        let Some(parsed) = regex::parse(&allocator, ctx, lit) else {
            return;
        };
        let Some(lazy) = trailing_lazy(&parsed.body, 0) else {
            return;
        };
        let message = format!(
            "The lazy '{}' ends this searched pattern, so it always matches {}: add what should end the match, or make it greedy",
            snippet(ctx.text(lazy.span)),
            minimum(ctx, lazy)
        );
        ctx.emit(lazy.span, message);
    }
}

/// The regular expression literal a call searches with: the first argument of `replace`,
/// `split`, … or the object of `exec` / `test`.
fn searched_literal<'b, 'a>(call: &'b CallExpression<'a>) -> Option<&'b RegExpLiteral<'a>> {
    let member = call.callee.get_inner_expression().as_member_expression()?;
    let searched = match member.static_property_name()? {
        "exec" | "test" => member.object(),
        "replace" | "replaceAll" | "split" | "match" | "matchAll" | "search" => {
            call.arguments.first()?.as_expression()?
        }
        _ => return None,
    };
    match helpers::unwrap_expr(searched) {
        Expression::RegExpLiteral(lit) => Some(lit),
        _ => None,
    }
}

/// The lazy, variable-count quantifier the disjunction ends with, if any: the last term of any
/// alternative, looking into (unquantified) groups.
fn trailing_lazy<'b, 'a>(d: &'b Disjunction<'a>, depth: usize) -> Option<&'b Quantifier<'a>> {
    if depth > MAX_DEPTH {
        return None;
    }
    d.body
        .iter()
        .find_map(|alternative| match alternative.body.last()? {
            Term::Quantifier(q) if !q.greedy && q.max != Some(q.min) => match &q.body {
                // A quantified assertion is `js/useless-regex-quantifier`'s case.
                Term::BoundaryAssertion(_) | Term::LookAroundAssertion(_) => None,
                _ => Some(&**q),
            },
            Term::CapturingGroup(g) => trailing_lazy(&g.body, depth + 1),
            Term::IgnoreGroup(g) => trailing_lazy(&g.body, depth + 1),
            _ => None,
        })
}

/// What the lazy quantifier always matches, in words.
fn minimum(ctx: &Ctx<'_>, q: &Quantifier<'_>) -> String {
    let what = snippet(ctx.text(q.body.span()));
    match q.min {
        0 => "nothing".to_string(),
        1 => format!("a single '{what}'"),
        n => format!("exactly {n} of '{what}'"),
    }
}

/// `text`, shortened for a message.
fn snippet(text: &str) -> Cow<'_, str> {
    const MAX: usize = 30;
    match text.char_indices().nth(MAX) {
        Some((i, _)) => Cow::Owned(format!("{}…", &text[..i])),
        None => Cow::Borrowed(text),
    }
}
