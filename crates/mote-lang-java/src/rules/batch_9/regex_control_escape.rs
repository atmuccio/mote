//! `java/regex-control-escape` — a `\c` control escape in a constant regular expression that
//! does not name a control character (`\ca`, `\c1`).

use super::regex_call::{self, CALL_NAMES};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["regex"],
    remediation_minutes: 2,
    description: r#"Detects a `\c` control escape in a constant regular expression followed by a
character other than `@`, an upper-case ASCII letter, `[`, `\`, `]`, `^`, `_` or `?`: `\ca`,
`\cz`, `\c1`.

**Why it matters.** Java computes `\cX` by flipping one bit of the next character (`X ^ 64`),
whatever that character is. Only `@`, `A`–`Z` and `[ \ ] ^ _` give the control characters
`\x00`–`\x1F` (and `?` gives DEL); `\ca` is the character `!`, not Ctrl-A, and `\c1` is the
letter `q`. Other regex dialects accept the lower-case form, so the pattern looks right but
looks for a printable character instead of the control character.

**How to fix.** Write the letter in upper case (`\cA`), or use a hexadecimal escape
(`\x01`).

```java
// Noncompliant
Pattern bell = Pattern.compile("\\cg");   // matches an apostrophe (0x67 ^ 0x40), not BEL

// Compliant
Pattern bell = Pattern.compile("\\cG");   // U+0007
Pattern bel2 = Pattern.compile("\\x07");
```

Only constant patterns are checked; text inside `\Q…\E` is literal and ignored."#,
    ..RuleMeta::new(
        "java/regex-control-escape",
        "java",
        "Regex control escape that is not a control character",
    )
};

pub struct RegexControlEscape;

/// The first `\c` escape of `pattern` whose letter is not one of `?@A-Z[\]^_`, with the
/// character it actually denotes.
fn bad_control_escape(pattern: &str) -> Option<(char, char)> {
    let mut it = pattern.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\\' {
            continue;
        }
        match it.next()? {
            'Q' => {
                // Literal text up to `\E` (or the end).
                let mut prev = '\0';
                for q in it.by_ref() {
                    if prev == '\\' && q == 'E' {
                        break;
                    }
                    prev = q;
                }
            }
            'c' => {
                let x = it.next()?;
                if !('?'..='_').contains(&x) {
                    let actual = char::from_u32((x as u32) ^ 64)?;
                    return Some((x, actual));
                }
            }
            _ => {}
        }
    }
    None
}

/// How to show a character in a message: printable ASCII as is, anything else as `U+XXXX`.
fn show(c: char) -> String {
    if c.is_ascii_graphic() {
        format!("`{c}`")
    } else {
        format!("U+{:04X}", c as u32)
    }
}

impl<'t> JavaRule<'t> for RegexControlEscape {
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
        let Some(u) = regex_call::regex_use(ctx, node, |p| p.contains("\\c")) else {
            return;
        };
        let Some((x, actual)) = bad_control_escape(&u.pattern) else {
            return;
        };
        // The pattern must compile: a broken one is `java/invalid-regex-literal`'s.
        if u.parse().is_none() {
            return;
        }
        let fix = if x.is_ascii_lowercase() {
            format!("write `\\c{}`", x.to_ascii_uppercase())
        } else {
            "use a `\\xHH` escape".to_string()
        };
        ctx.emit(
            u.node,
            format!(
                "Fix the control escape `\\c{x}`: Java flips one bit of the next character, so it matches {} instead of a control character; {fix}",
                show(actual)
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::bad_control_escape;

    #[test]
    fn control_escapes() {
        assert_eq!(bad_control_escape(r"\ca"), Some(('a', '!')));
        assert_eq!(bad_control_escape(r"x\c1"), Some(('1', 'q')));
        assert_eq!(bad_control_escape(r"\cA\cZ\c@\c[\c_\c?"), None);
        assert_eq!(bad_control_escape(r"\\ca"), None);
        assert_eq!(bad_control_escape(r"\Q\ca\E"), None);
        assert_eq!(bad_control_escape(r"\Q\ca\E\cb"), Some(('b', '"')));
        assert_eq!(bad_control_escape(r"\c"), None);
        assert_eq!(bad_control_escape(r"[\cm]"), Some(('m', '-')));
    }
}
