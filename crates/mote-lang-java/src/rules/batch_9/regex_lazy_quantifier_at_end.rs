//! `java/regex-lazy-quantifier-at-end` — a lazy quantifier that ends a searched pattern, so it
//! always matches its minimum.

use super::regex_call::{self, Usage, CALL_NAMES};
use super::regex_syntax::{GroupKind, Mode, Regex, Rx, K};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["regex"],
    remediation_minutes: 5,
    description: r#"Detects a lazy quantifier (`*?`, `+?`, `??`, `{n,m}?`) at the very end of a
constant regular expression that is searched for in the input — `split`, `replaceAll`,
`replaceFirst`, or `Pattern.compile(..)` followed directly by `matcher(..).find()`, `split`
and the like: `"<!--.*?"`, `"key=\\w+?"`.

**Why it matters.** A lazy quantifier takes as few repetitions as it can and only takes more
when what follows it fails to match. At the end of a searched pattern nothing follows, so it
always stops at its minimum: `.*?` matches nothing, `\w+?` exactly one character, `{2,5}?`
exactly two. The pattern looks as if it captures "the rest", but it never does; usually the
part that should end the match (`-->`, a delimiter, `$`) was forgotten.

**How to fix.** Write what ends the match after the quantifier (`<!--.*?-->`), or make the
quantifier greedy if the longest run was meant.

```java
// Noncompliant
String plain = html.replaceAll("<!--.*?", "");   // removes only "<!--"

// Compliant
String plain = html.replaceAll("<!--.*?-->", "");
```

Not reported: patterns matched as a whole (`matches`, `@Pattern`), where the end of the input
forces the quantifier to go on, and compiled patterns whose use is not visible at the
`compile` call."#,
    ..RuleMeta::new(
        "java/regex-lazy-quantifier-at-end",
        "java",
        "Lazy quantifier at the end of a searched regular expression",
    )
};

pub struct RegexLazyQuantifierAtEnd;

/// The lazy repetition that `rx` ends with, if any: the last item of a sequence, the body of a
/// capturing or non-capturing group, or the end of any alternative.
fn trailing_lazy(rx: &Rx) -> Option<&Rx> {
    match &rx.k {
        K::Repeat {
            mode: Mode::Lazy,
            max,
            min,
            ..
        } if *max != Some(*min) => Some(rx),
        K::Group(GroupKind::Capture(_) | GroupKind::NonCapture, body) => trailing_lazy(body),
        K::Seq(items) => trailing_lazy(items.last()?),
        K::Alt(branches) => branches.iter().find_map(trailing_lazy),
        _ => None,
    }
}

/// What the lazy repetition `rx` always matches, in words.
fn minimum(re: &Regex, rx: &Rx) -> String {
    let K::Repeat { body, min, .. } = &rx.k else {
        return String::new();
    };
    let what = re.snippet(body);
    match min {
        0 => "nothing".to_string(),
        1 => format!("a single `{what}`"),
        n => format!("exactly {n} of `{what}`"),
    }
}

impl<'t> JavaRule<'t> for RegexLazyQuantifierAtEnd {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation", "annotation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        CALL_NAMES
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(u) = regex_call::regex_use(ctx, node, |p| p.contains('?')) else {
            return;
        };
        if u.usage != Usage::Find {
            return;
        }
        let Some(re) = u.parse() else {
            return;
        };
        let Some(lazy) = trailing_lazy(&re.root) else {
            return;
        };
        ctx.emit(
            u.node,
            format!(
                "The lazy `{}` ends this searched pattern, so it always matches {}: add what should end the match, or make it greedy",
                re.snippet(lazy),
                minimum(&re, lazy)
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::super::regex_syntax::parse;
    use super::trailing_lazy;

    #[test]
    fn trailing_lazy_quantifiers() {
        let t = |p: &str| trailing_lazy(&parse(p, 0).unwrap().root).is_some();
        assert!(t("<!--.*?"));
        assert!(t(r"key=\w+?"));
        assert!(t("a(b.*?)"));
        assert!(t("x|y+?"));
        assert!(t("a{2,5}?"));
        assert!(t("a??"));
        assert!(!t("<!--.*?-->"));
        assert!(!t(".*?$"));
        assert!(!t("a{2}?"));
        assert!(!t(r"a+?\b"));
        assert!(!t("(?=a+?)"));
        assert!(!t("a*"));
        assert!(!t(r"a\?"));
    }
}
