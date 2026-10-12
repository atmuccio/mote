//! `java/regex-empty-alternative` — an empty alternative in a constant regular expression.

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
    description: r#"Detects an explicit empty alternative in a constant regular expression:

* two `|` in a row (`a||b`), anywhere: the gap is an alternative that matches the empty
  string;
* a leading or trailing `|` at the top of a pattern that is searched for — `split`,
  `replaceAll` / `replaceFirst` with a non-empty replacement, or `Pattern.compile(..)`
  followed directly by `split`, `matcher(..).find()` and the like (`",|;|"`).

**Why it matters.** A pattern with an empty alternative matches the empty string at every
position. `"a,b".split(",|")` does not split on commas only: it cuts the input between every
pair of characters. `replaceAll("x|", "-")` inserts `-` everywhere. `||` usually comes from
thinking of the logical OR operator.

**How to fix.** Remove the stray `|`; if an optional part was meant, use `?`.

```java
// Noncompliant
String[] parts = line.split(",|;|");
String cleaned = text.replaceAll("\\s||-", "_");

// Compliant
String[] parts = line.split(",|;");
String cleaned = text.replaceAll("\\s|-", "_");
```

Not reported: a leading or trailing empty alternative inside a group (`(a|)`, an optional
part), in a pattern matched as a whole (`matches`, `@Pattern`), or in a `replaceAll` whose
replacement is the empty string (the extra empty matches then change nothing)."#,
    ..RuleMeta::new(
        "java/regex-empty-alternative",
        "java",
        "Empty alternative in a regular expression",
    )
};

pub struct RegexEmptyAlternative;

impl<'t> JavaRule<'t> for RegexEmptyAlternative {
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
        if let Some(msg) =
            regex_checks::empty_alternative(&re, u.usage, u.split, u.empty_replacement)
        {
            ctx.emit(u.node, msg);
        }
    }
}
