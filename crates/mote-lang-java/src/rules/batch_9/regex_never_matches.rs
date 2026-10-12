//! `java/regex-never-matches` — a constant regular expression with a part that can never match.

use super::regex_call::{self, CALL_NAMES};
use super::regex_checks;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["regex"],
    remediation_minutes: 10,
    description: r#"Detects a constant regular expression — passed to `Pattern.compile`,
`Pattern.matches`, `String.matches` / `replaceAll` / `replaceFirst` / `split`, or given to a
Bean Validation `@Pattern` — containing a part that can never match:

* an anchor in a place it cannot hold: `^` or `\A` after text that must already have been
  matched (`a^b`), `$` followed by a character that is not a line break (`$abc`), `\z`
  followed by more text;
* a word boundary between two characters of the same kind (`\w\b\w`, `,\b `), or `\B`
  between a word and a non-word character;
* a lookaround contradicting its surroundings: `(?=\d)[a-z]`, `(?!a)a`, `a(?<=b)`;
* a back-reference to a group that does not exist (`(a)\2`) or that cannot have matched yet
  (`\1(a)`, `(a)|\1`, `(a\1)`);
* an unbounded possessive repetition followed by a character it would have taken (`\d++\d`,
  `[a-z]*+s`): it keeps every such character and never gives one back;
* a `\b` written in the Java string (`"\bword"`): that is a backspace character, not a word
  boundary.

**Why it matters.** The pattern compiles, so nothing fails loudly: the branch or the whole
pattern simply never matches, and the code silently takes the "no match" path — validation
that rejects everything, a `split` that never splits, a `replaceAll` that changes nothing.

**How to fix.** Move the anchor to where it can hold (or enable `MULTILINE` if line
anchors were meant), fix the lookaround or the group number, and write `\\b` in Java
strings for a word boundary.

```java
// Noncompliant
Pattern.compile("^\\d+^");          // the second ^ can never match
Pattern.compile("\\d+$px");         // $ followed by more text
line.matches("\\bid\\b=(\\d+)\\2"); // there is no group 2

// Compliant
Pattern.compile("^\\d+$");
Pattern.compile("\\d+px$");
line.matches("\\bid\\b=(\\d+)");
```

Only constant patterns are checked (string literals, possibly concatenated). Case-insensitive
and Unicode-class modes are taken into account; patterns using comments mode, `LITERAL`,
`CANON_EQ`, or constructs the checker does not model are skipped, as are syntax errors (see
`java/invalid-regex-literal`)."#,
    ..RuleMeta::new(
        "java/regex-never-matches",
        "java",
        "Regular expression part that can never match",
    )
};

pub struct RegexNeverMatches;

impl<'t> JavaRule<'t> for RegexNeverMatches {
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
        let Some(u) = regex_call::regex_use(ctx, node, regex_checks::may_never_match) else {
            return;
        };
        let Some(re) = u.parse() else {
            return;
        };
        if regex_checks::has_backspace_escape(ctx.text(u.node))
            && regex_checks::has_backspace_char(&re)
        {
            ctx.emit(
                u.node,
                "Write `\\\\b` for a word boundary: `\\b` in a Java string is a backspace character, which the pattern then looks for literally",
            );
            return;
        }
        if let Some(msg) = regex_checks::never_matches(&re) {
            ctx.emit(u.node, msg);
        }
    }
}
