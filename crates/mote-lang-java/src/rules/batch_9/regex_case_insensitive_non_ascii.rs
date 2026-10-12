//! `java/regex-case-insensitive-non-ascii` — case-insensitive matching of non-ASCII letters
//! without `UNICODE_CASE`.

use super::regex_call::{self, CALL_NAMES};
use super::regex_checks;
use super::regex_syntax::CASE_INSENSITIVE;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["regex", "i18n"],
    remediation_minutes: 2,
    description: r#"Detects a constant regular expression compiled case-insensitively — the
`CASE_INSENSITIVE` flag or inline `(?i)` — without `UNICODE_CASE`, that contains a non-ASCII
letter having another case form (`é`/`É`, `ä`/`Ä`, Cyrillic, Greek…).

**Why it matters.** By default, Java's case-insensitive matching only folds US-ASCII
letters. `Pattern.compile("café", CASE_INSENSITIVE)` matches `CAFé` but not `CAFÉ`, and
`(?i)[а-я]+` does not match capital Cyrillic letters. The pattern looks right and works on
the test data, then misses real input.

**How to fix.** Add `Pattern.UNICODE_CASE` to the flags (`(?iu)` inline), or list both cases
explicitly.

```java
// Noncompliant
Pattern p = Pattern.compile("^straße|münchen$", Pattern.CASE_INSENSITIVE);

// Compliant
Pattern p = Pattern.compile("^straße|münchen$", Pattern.CASE_INSENSITIVE | Pattern.UNICODE_CASE);
```

Not reported: character classes that already list the other case (`[äÄ]`), negated classes,
letters whose other case is several characters (`ß`), and patterns whose flags are not
constant."#,
    ..RuleMeta::new(
        "java/regex-case-insensitive-non-ascii",
        "java",
        "Case-insensitive regex with non-ASCII letters but no Unicode case",
    )
};

pub struct RegexCaseInsensitiveNonAscii;

impl<'t> JavaRule<'t> for RegexCaseInsensitiveNonAscii {
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
        let Some(u) = regex_call::regex_use(ctx, node, |p| !p.is_ascii()) else {
            return;
        };
        if u.pattern.is_ascii() || (u.flags & CASE_INSENSITIVE == 0 && !u.pattern.contains("(?")) {
            return;
        }
        let Some(re) = u.parse() else {
            return;
        };
        if let Some(msg) = regex_checks::ascii_only_case_folding(&re) {
            ctx.emit(u.node, msg);
        }
    }
}
