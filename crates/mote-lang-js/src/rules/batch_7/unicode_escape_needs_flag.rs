//! `js/unicode-escape-needs-flag`

use super::regex;
use crate::prelude::*;

pub struct UnicodeEscapeNeedsFlag;

pub static UNICODE_ESCAPE_NEEDS_FLAG: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["regex", "suspicious"],
    remediation_minutes: 1,
    description: r#"Flags a Unicode property escape (`\p{…}`, `\P{…}`) or a code point escape (`\u{…}`) in a regular
expression literal without the `u` or `v` flag.

**Why:** these escapes only exist in Unicode mode. Without the flag, the backslash is simply
dropped: `/\p{L}+/` matches the text `p{L}` (one or more `}`), not letters, and `/\u{1F600}/`
matches the letter `u` followed by `{1F600}`. The expression is valid, so nothing warns — it just
never matches what it says.

**Fix:** add the `u` flag (or `v`).

```js
/^\p{Lu}/.test(name)      // flagged: matches the text "p{Lu}"
/\u{1F600}/               // flagged
/^\p{Lu}/u.test(name)     // compliant
```
"#,
    ..RuleMeta::new(
        "js/unicode-escape-needs-flag",
        "js",
        "Unicode escapes in regular expressions need the 'u' flag",
    )
};

impl JsRule for UnicodeEscapeNeedsFlag {
    fn meta(&self) -> &'static RuleMeta {
        &UNICODE_ESCAPE_NEEDS_FLAG
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::RegExpLiteral]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::RegExpLiteral(lit) = node.kind() else {
            return;
        };
        if regex::is_unicode(lit) {
            return;
        }
        let pattern = lit.regex.pattern.text.as_str();
        if !pattern.contains("{") || !pattern.contains('\\') {
            return;
        }
        let Some(offset) = first_unicode_escape(pattern) else {
            return;
        };
        let start = lit.span.start + 1 + offset as u32;
        let escape: String = pattern[offset..]
            .chars()
            .take_while(|&c| c != '}')
            .chain(std::iter::once('}'))
            .collect();
        let digits = escape
            .strip_prefix("\\u{")
            .and_then(|s| s.strip_suffix('}'))
            .filter(|d| d.bytes().all(|b| b.is_ascii_digit()));
        let meaning = match digits {
            Some(n) => format!("the letter 'u' repeated {n} times"),
            None => format!("the literal text '{}'", &escape[1..]),
        };
        ctx.emit(
            Span::new(start, start + escape.len() as u32),
            format!(
                "Add the 'u' flag: without it, '{escape}' is not an escape and matches {meaning}"
            ),
        );
    }
}

/// Byte offset of the first `\p{…}`, `\P{…}` or `\u{hex}` whose backslash is not itself escaped.
fn first_unicode_escape(pattern: &str) -> Option<usize> {
    let bytes = pattern.as_bytes();
    let mut i = 0;
    while i + 2 < bytes.len() {
        if bytes[i] != b'\\' {
            i += 1;
            continue;
        }
        let letter = bytes[i + 1];
        if bytes[i + 2] == b'{' {
            let close = pattern[i + 3..].find('}').map(|c| i + 3 + c);
            if let Some(close) = close {
                let inner = &pattern[i + 3..close];
                let valid = match letter {
                    b'p' | b'P' => {
                        inner.starts_with(|c: char| c.is_ascii_alphabetic())
                            && inner
                                .bytes()
                                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'=')
                    }
                    b'u' => !inner.is_empty() && inner.bytes().all(|b| b.is_ascii_hexdigit()),
                    _ => false,
                };
                if valid {
                    return Some(i);
                }
            }
        }
        // Skip the escaped character (so `\\p{L}` is a backslash followed by `p{L}`).
        i += 2;
    }
    None
}
