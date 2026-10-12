//! `java/regex-stack-overflow-risk` — a repeated alternation of single characters, which
//! `java.util.regex` matches recursively.

use super::regex_call::{self, CALL_NAMES};
use super::regex_checks;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["regex", "performance"],
    remediation_minutes: 5,
    description: r#"Detects a constant regular expression that repeats, without an upper
bound, a group alternating between single characters: `(a|b)*`, `(?:\s|,)+`, and the classic
"anything including newlines" `(.|\n)*`.

**Why it matters.** `java.util.regex` is a backtracking matcher. A repeated group containing
an alternation is matched recursively, one level of Java stack per iteration, so the depth
grows with the length of the input. Input of a few thousand characters — a large text field,
a file, a request body — throws `StackOverflowError`, which is usually not caught and kills
the thread. A character class matches the same text iteratively.

**How to fix.** Replace the alternation with a character class (`[ab]*`, `[\s,]+`), use
`[\s\S]*` or the `DOTALL` flag for "anything", or make the repetition possessive (`*+`) when
backtracking into it is not needed.

```java
// Noncompliant
Pattern block = Pattern.compile("BEGIN(.|\\n)*END");
boolean ok = code.matches("(\\d|-)+");

// Compliant
Pattern block = Pattern.compile("BEGIN.*END", Pattern.DOTALL);
boolean ok = code.matches("[\\d-]+");
```

Not reported: bounded repetitions (`{1,5}`), possessive ones, and alternations with longer
alternatives (`(ab|cd)*`), which cannot become a character class."#,
    ..RuleMeta::new(
        "java/regex-stack-overflow-risk",
        "java",
        "Repeated alternation that can overflow the stack",
    )
};

pub struct RegexStackOverflowRisk;

impl<'t> JavaRule<'t> for RegexStackOverflowRisk {
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
        let Some(u) = regex_call::regex_use(ctx, node, |p| {
            p.contains('|') && (p.contains('*') || p.contains('+') || p.contains(",}"))
        }) else {
            return;
        };
        if !u.pattern.contains('|') {
            return;
        }
        let Some(re) = u.parse() else {
            return;
        };
        if let Some(msg) = regex_checks::overflow_risk(&re) {
            ctx.emit(u.node, msg);
        }
    }
}
