//! `java/numeric-escape-for-named-char` — `"\11"` or `'\u0009'` where the short escape `\t`
//! exists.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["conventions", "strings"],
    remediation_minutes: 1,
    description: r#"Detects octal escapes (`\11`, `\12`, `\42`, `\134`, …) and Unicode escapes
(`\u0009`, `\u0008`, `\u000c`) in string, text block and character literals for characters
that have a short named escape: backspace `\b`, tab `\t`, line feed `\n`, form feed `\f`,
carriage return `\r`, double quote `\"`, single quote `\'` and backslash `\\`.

**Why it matters.** Few readers know that `\12` is a line feed or `\42` a double quote; the
named escapes say what the character is. A Unicode escape is worse: it is replaced before the
code is even tokenized, so `"\u0009"` is a raw tab inside the literal, and the same trick with
`\u000a` or `"` does not even compile.

**How to fix.** Use the named escape.

```java
// Noncompliant
String row = name + "\11" + value + "\12";
char tab = '\u0009';

// Compliant
String row = name + "\t" + value + "\n";
char tab = '\t';
```

Not reported: numeric escapes for characters without a named escape (`\0`, `\u001b`), escaped
backslashes followed by digits (`"\\12"`), and the space character (its named escape `\s`
needs Java 15). The rule is off by default: it is part of the `conventions` profile."#,
    ..RuleMeta::new(
        "java/numeric-escape-for-named-char",
        "java",
        "Numeric escape for a character with a named escape",
    )
};

pub struct NumericEscapeForNamedChar;

/// The named escape for an escape sequence written numerically, with the numeric part.
fn named_escape(esc: &str) -> Option<(&'static str, &str)> {
    let b = esc.as_bytes();
    if b.first() != Some(&b'\\') {
        return None;
    }
    let first = *b.get(1)?;
    if (b'0'..=b'7').contains(&first) {
        let run = b[1..]
            .iter()
            .take_while(|c| (b'0'..=b'7').contains(*c))
            .count();
        let take = run.min(if first <= b'3' { 3 } else { 2 });
        let digits = &esc[1..1 + take];
        let value = u32::from_str_radix(digits, 8).ok()?;
        let named = match value {
            8 => "\\b",
            9 => "\\t",
            10 => "\\n",
            12 => "\\f",
            13 => "\\r",
            34 => "\\\"",
            39 => "\\'",
            92 => "\\\\",
            _ => return None,
        };
        return Some((named, &esc[..1 + take]));
    }
    if first == b'u' {
        let us = b[1..].iter().take_while(|c| **c == b'u').count();
        let hex = esc.get(1 + us..1 + us + 4)?;
        let value = u32::from_str_radix(hex, 16).ok()?;
        let named = match value {
            8 => "\\b",
            9 => "\\t",
            12 => "\\f",
            _ => return None,
        };
        return Some((named, &esc[..1 + us + 4]));
    }
    None
}

impl<'t> JavaRule<'t> for NumericEscapeForNamedChar {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["escape_sequence", "character_literal"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let text = ctx.text(node);
        let esc = if kind(node) == "character_literal" {
            text.strip_prefix('\'')
                .and_then(|t| t.strip_suffix('\''))
                .unwrap_or("")
        } else {
            text
        };
        if esc.len() < 2 {
            return;
        }
        let Some((named, numeric)) = named_escape(esc) else {
            return;
        };
        ctx.emit(
            node,
            format!("Write `{named}` instead of the numeric escape `{numeric}`"),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::named_escape;

    #[test]
    fn escapes() {
        assert_eq!(named_escape("\\11"), Some(("\\t", "\\11")));
        assert_eq!(named_escape("\\012"), Some(("\\n", "\\012")));
        assert_eq!(named_escape("\\134"), Some(("\\\\", "\\134")));
        assert_eq!(named_escape("\\42"), Some(("\\\"", "\\42")));
        assert_eq!(named_escape("\\0"), None);
        assert_eq!(named_escape("\\40"), None);
        assert_eq!(named_escape("\\u0009"), Some(("\\t", "\\u0009")));
        assert_eq!(named_escape("\\uu000C"), Some(("\\f", "\\uu000C")));
        assert_eq!(named_escape("\\u001b"), None);
        assert_eq!(named_escape("\\t"), None);
        assert_eq!(named_escape("\\u00"), None);
    }
}
