//! `java/regex-suspicious-character-class` — a character class of a constant regular
//! expression that does not mean what it seems to.

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
    description: r#"Detects character classes of constant regular expressions that match
something else than they appear to:

* a range from an upper-case to a lower-case letter, or from a digit to a letter (`[A-z]`,
  `[0-z]`): it also covers the ASCII punctuation in between (`[`, `\`, `]`, `^`, `_`, the
  backtick; `:` to `@`);
* a letter followed by a combining mark (`é` written as `e` + U+0301, an emoji with a
  modifier): the class holds two separate members, each matched on its own;
* `|` used as if it separated alternatives, with repeated characters (`[jpg|png]`): in a
  class `|` is an ordinary character and the class matches one character.

**Why it matters.** The pattern accepts input it should reject (`[A-z]+` accepts `_^[]`,
`[jpg|png]` accepts a lone `p` or `|`) or splits a user-perceived character in two.

**How to fix.** Write the ranges separately (`[A-Za-z]`), use a group for alternatives
(`(?:jpg|png)`), and put multi-code-point characters in an alternation instead of a class.

```java
// Noncompliant
boolean ok = name.matches("[A-z0-9]+");
Pattern images = Pattern.compile(".*\\.[jpg|png]");

// Compliant
boolean ok = name.matches("[A-Za-z0-9]+");
Pattern images = Pattern.compile(".*\\.(?:jpg|png)");
```

Not reported: classes with nesting or intersections (`[a-z&&[^e]]`), ranges between
punctuation characters (`[!-~]`), and a single `|` among distinct characters (`[|,;]`),
which is a legitimate way to list separators."#,
    ..RuleMeta::new(
        "java/regex-suspicious-character-class",
        "java",
        "Suspicious character class in a regular expression",
    )
};

pub struct RegexSuspiciousCharacterClass;

impl<'t> JavaRule<'t> for RegexSuspiciousCharacterClass {
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
        let Some(u) = regex_call::regex_use(ctx, node, |p| p.contains('[')) else {
            return;
        };
        if !u.pattern.contains('[') {
            return;
        }
        let Some(re) = u.parse() else {
            return;
        };
        if let Some(msg) = regex_checks::class_pitfall(&re) {
            ctx.emit(u.node, msg);
        }
    }
}
