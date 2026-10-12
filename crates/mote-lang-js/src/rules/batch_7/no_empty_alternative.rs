//! `js/no-empty-alternative`

use super::regex;
use crate::prelude::*;

pub struct NoEmptyAlternative;

pub static NO_EMPTY_ALTERNATIVE: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["regex", "suspicious"],
    remediation_minutes: 2,
    description: r#"Flags an empty alternative in a regular expression that makes it match where it should not: a
stray `|` at the start or end of the whole pattern (`/foo|/`), or a doubled `||` between two
alternatives (`/(png||jpg)/`).

**Why:** an empty alternative matches the empty string. At the top level of the pattern it matches
at every position of every input, so `test()` is always `true` and `replace()` inserts its
replacement between all characters. Inside a group, `||` between two alternatives silently makes
the whole group optional. Both are almost always a typo — a leftover separator or a doubled pipe.

**Fix:** remove the extra `|`; if the group really is optional, say so with `?`
(`(png|jpg)?`).

```js
/error|warning|/.test(line)    // flagged: always true
/\.(png||jpg)$/                // flagged: the extension is optional
/error|warning/.test(line)     // compliant
/\.(png|jpg)?$/                // compliant
```

An empty first or last alternative inside a group (`(|s)`, `(s|)`) is a common, deliberate way of
writing "optional" and is not reported. Only regular expression literals are checked.
"#,
    ..RuleMeta::new(
        "js/no-empty-alternative",
        "js",
        "Regular expressions should not contain stray empty alternatives",
    )
};

impl JsRule for NoEmptyAlternative {
    fn meta(&self) -> &'static RuleMeta {
        &NO_EMPTY_ALTERNATIVE
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
        let mut found: Vec<(Span, &'static str)> = Vec::new();
        regex::for_each_disjunction(&pattern, &mut |disjunction, top| {
            let alternatives = &disjunction.body;
            let n = alternatives.len();
            if n < 2 {
                return;
            }
            for (k, alternative) in alternatives.iter().enumerate() {
                if !alternative.body.is_empty() || (!top && (k == 0 || k == n - 1)) {
                    continue;
                }
                // The `|` next to the empty alternative: before it, or after it for the first.
                let at = alternative.span.start;
                let bar = if k == 0 { at } else { at.saturating_sub(1) };
                if ctx.text(Span::new(bar, bar + 1)) != "|"
                    || found.last().is_some_and(|(s, _)| s.start == bar)
                {
                    continue;
                }
                let message = if top {
                    "Remove this '|': the empty alternative next to it matches the empty string anywhere, so the whole expression matches every input"
                } else {
                    "Remove the doubled '|': the empty alternative between the two makes this group optional"
                };
                found.push((Span::new(bar, bar + 1), message));
            }
        });
        for (span, message) in found {
            ctx.emit(span, message);
        }
    }
}
