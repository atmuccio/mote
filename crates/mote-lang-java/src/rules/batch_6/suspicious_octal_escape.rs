//! `java/suspicious-octal-escape` — `"\128"` is `\12` (a line feed) followed by `8`.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Low,
    status: RuleStatus::Incubating,
    tags: &["suspicious", "strings"],
    remediation_minutes: 2,
    description: r#"Detects octal escapes in string literals that are directly followed by a
digit the escape does not include, such as `"\128"` or `"\477"`.

**Why it matters.** An octal escape takes at most three octal digits, and at most two when the
first is `4`–`7`. `"\128"` is therefore `\12` (a line feed) followed by the character `8`, and
`"\477"` is `'` followed by `7`. Whoever wrote the literal most likely meant one character
with a larger code, or a decimal number — either way the string is not what it looks like.

**How to fix.** Use a Unicode escape (`\u0080`) for the character you mean, split the literal
so the digit is clearly separate (`"\12" + "8"`), or write the digit without an escape.

```java
// Noncompliant
String sep = "\128";

// Compliant
String sep = "\n8";
String high = "\u0080";
```

Not reported: complete octal escapes such as `"\0"`, `"\12"`, or `"\177"` followed by a
non-digit, escaped backslashes (`"\\12"`, a regex back-reference), character literals, and
text blocks."#,
    ..RuleMeta::new(
        "java/suspicious-octal-escape",
        "java",
        "Octal escape followed by a digit",
    )
};

pub struct SuspiciousOctalEscape;

impl<'t> JavaRule<'t> for SuspiciousOctalEscape {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["escape_sequence"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let text = ctx.text(node).as_bytes();
        // `\` followed by an octal digit.
        let Some(&first) = text.get(1) else {
            return;
        };
        if text.first() != Some(&b'\\') || !(b'0'..=b'7').contains(&first) {
            return;
        }
        let run = text[1..]
            .iter()
            .take_while(|c| (b'0'..=b'7').contains(*c))
            .count();
        let max = if first <= b'3' { 3 } else { 2 };
        let taken = run.min(max);
        // The parser may read more digits into the token than Java does (`\477`); otherwise
        // look at the character right after the token.
        let stray = if run > taken {
            text.get(1 + taken).copied()
        } else {
            ctx.source()
                .as_bytes()
                .get(node.end_byte())
                .copied()
                .filter(u8::is_ascii_digit)
        };
        let Some(digit) = stray else {
            return;
        };
        // Only inside string literals (a character literal ends right after the escape).
        if ctx.parent(node).is_none_or(|p| kind(p) != "string_literal") {
            return;
        }
        let escape = String::from_utf8_lossy(&text[..1 + taken]);
        ctx.emit(
            node,
            format!(
                "Clarify this escape: `{escape}` ends before the digit `{}`, which is a separate \
                 character",
                digit as char
            ),
        );
    }
}
