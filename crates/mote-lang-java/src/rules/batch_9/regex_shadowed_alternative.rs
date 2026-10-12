//! `java/regex-shadowed-alternative` — an alternative of a constant regular expression that is
//! never used.

use super::regex_call::{self, CALL_NAMES};
use super::regex_checks;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["regex"],
    remediation_minutes: 5,
    description: r#"Detects an alternative of a constant regular expression that can never be
chosen:

* an exact duplicate of an earlier alternative of the same alternation (`jpg|png|jpg`);
* where the pattern is searched for (`split`, `replaceAll`, `replaceFirst`, or a compiled
  pattern used right away with `split` / `matcher(..).find()`), a literal alternative that
  starts with an earlier literal alternative, when nothing follows the alternation
  (`",|, "`): Java tries alternatives from left to right and stops at the first that matches,
  so the shorter one always wins.

**Why it matters.** The longer alternative was written for a reason — to also consume the
space after a comma, to replace a whole word — and it silently never takes effect:
`"a, b".split(",|, ")` leaves a leading space on `" b"`. A duplicate alternative is dead
code that usually hides a typo in the one that was meant.

**How to fix.** Put longer alternatives first (`", |,"`), or make the difference optional
(`",\\s?"`); remove duplicates.

```java
// Noncompliant
String[] parts = line.split(";|;\\s");
String out = text.replaceAll("cat|dog|cat", "pet");

// Compliant
String[] parts = line.split(";\\s|;");
String out = text.replaceAll("cat|dog", "pet");
```

Not reported: prefix-shadowed alternatives in patterns matched as a whole (`matches`,
`@Pattern`: when the short alternative leaves input unmatched, Java backtracks into the
longer one) or followed by more pattern (`(a|ab)c`); duplicates involving capturing groups,
back-references or inline flags, whose meaning can differ between the copies."#,
    ..RuleMeta::new(
        "java/regex-shadowed-alternative",
        "java",
        "Regular expression alternative that is never used",
    )
};

pub struct RegexShadowedAlternative;

impl<'t> JavaRule<'t> for RegexShadowedAlternative {
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
        let Some(u) = regex_call::regex_use(ctx, node, |p| p.contains('|')) else {
            return;
        };
        if !u.pattern.contains('|') {
            return;
        }
        let Some(re) = u.parse() else {
            return;
        };
        if let Some(msg) = regex_checks::shadowed_alternative(&re, u.usage) {
            ctx.emit(u.node, msg);
        }
    }
}
