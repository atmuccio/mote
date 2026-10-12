//! `js/regex-control-character`

use crate::prelude::*;

pub struct RegexControlCharacter;

pub static REGEX_CONTROL_CHARACTER: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["regex", "suspicious"],
    remediation_minutes: 2,
    description: r#"Flags a regular expression literal that contains a raw, invisible control character (U+0000 to
U+001F other than tab) typed directly into the pattern.

**Why:** the character is invisible in editors and reviews, so the pattern does not read as what it
matches; it usually got there by accident (pasted from a terminal or a binary file), and tools that
normalize or strip control characters change the pattern without anyone noticing.

**Fix:** write the character as an escape (`\x1b`, `\u0000`, `\cA`), which also documents the
intent, or remove it if it is not meant to be there.

```js
const ansi = /␛\[[0-9;]*m/g;     // flagged when ␛ is the raw ESC character
const ansi = /\x1b\[[0-9;]*m/g;  // compliant
```

Escapes for control characters are not reported: matching them on purpose (terminal colors,
binary protocols) is legitimate. A raw tab is not reported either.
"#,
    ..RuleMeta::new(
        "js/regex-control-character",
        "js",
        "Regular expressions should not contain raw control characters",
    )
};

impl JsRule for RegexControlCharacter {
    fn meta(&self) -> &'static RuleMeta {
        &REGEX_CONTROL_CHARACTER
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::RegExpLiteral]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::RegExpLiteral(lit) = node.kind() else {
            return;
        };
        let pattern = lit.regex.pattern.text.as_str();
        let Some((offset, byte)) = pattern
            .bytes()
            .enumerate()
            .find(|&(_, b)| b < 0x20 && b != b'\t')
        else {
            return;
        };
        // The pattern text starts right after the opening `/`.
        let start = lit.span.start + 1 + offset as u32;
        if ctx.text(Span::new(start, start + 1)).as_bytes() != [byte] {
            return;
        }
        ctx.emit(
            Span::new(start, start + 1),
            format!(
                "Write this invisible control character as an escape ('\\x{byte:02x}'), or remove it if it is not meant to be matched"
            ),
        );
    }
}
