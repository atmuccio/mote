//! `java/invisible-character` — invisible, control or bidirectional-override characters in the
//! source, and Unicode escapes outside literals.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["unicode", "confusing", "security"],
    remediation_minutes: 5,
    description: r#"Detects characters in the source that readers cannot see but the compiler
does:

* bidirectional control characters (U+202A–U+202E, U+2066–U+2069, U+200E, U+200F, U+061C)
  anywhere, comments included: they reorder how the following text is displayed, so code can
  look different from what is compiled;
* raw zero-width and other invisible characters (zero-width space and joiners, word joiner,
  soft hyphen, byte order mark inside the file, Hangul fillers), non-breaking spaces and raw
  control characters in string and character literals and in code;
* Unicode escapes (`\u0022`) outside string, character literals and comments, and escapes of
  line breaks in comments: the compiler translates them before anything else, so an escape
  can end a comment or a string early.

**Why it matters.** Two strings that print the same but differ by an invisible character
compare unequal; an identifier with a zero-width character is a different identifier; a
bidirectional override can make a comment appear to end where it doesn't. None of this shows
in a code review.

**How to fix.** Remove the character, or write it as an escape inside the literal
(`"\u00A0"`), where the intent is visible.

```java
// Noncompliant: the space before the euro sign is a raw no-break space (U+00A0)
String price = amount + " €";

// Compliant
String price = amount + "\u00A0€";
```

Not reported: escapes inside string and character literals (they keep the source ASCII),
other escapes in comments (the character they denote stays inside the comment), non-breaking
spaces, zero-width and control characters in comments (pasted documentation), a byte
order mark at the start of the file, and the end-of-file control character some old editors
append."#,
    ..RuleMeta::new(
        "java/invisible-character",
        "java",
        "Invisible or control character in source",
    )
};

pub struct InvisibleCharacter;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    /// Reorders the display of the text around it.
    Bidi,
    /// Takes no space or looks like a plain space.
    Invisible,
    /// A raw control character.
    Control,
}

fn classify(c: char) -> Option<Class> {
    match c {
        '\u{202A}'..='\u{202E}'
        | '\u{2066}'..='\u{2069}'
        | '\u{200E}'
        | '\u{200F}'
        | '\u{061C}' => Some(Class::Bidi),
        '\u{200B}'
        | '\u{200C}'
        | '\u{200D}'
        | '\u{2060}'..='\u{2064}'
        | '\u{FEFF}'
        | '\u{00AD}'
        | '\u{180E}'
        | '\u{3164}'
        | '\u{115F}'
        | '\u{1160}'
        | '\u{FFA0}'
        | '\u{00A0}'
        | '\u{202F}'
        | '\u{2007}' => Some(Class::Invisible),
        '\u{0}'..='\u{8}' | '\u{B}' | '\u{E}'..='\u{1F}' | '\u{7F}'..='\u{9F}' => {
            Some(Class::Control)
        }
        _ => None,
    }
}

fn describe(c: char) -> &'static str {
    match c {
        '\u{202A}' => "left-to-right embedding",
        '\u{202B}' => "right-to-left embedding",
        '\u{202C}' => "pop directional formatting",
        '\u{202D}' => "left-to-right override",
        '\u{202E}' => "right-to-left override",
        '\u{2066}' => "left-to-right isolate",
        '\u{2067}' => "right-to-left isolate",
        '\u{2068}' => "first strong isolate",
        '\u{2069}' => "pop directional isolate",
        '\u{200E}' => "left-to-right mark",
        '\u{200F}' => "right-to-left mark",
        '\u{061C}' => "Arabic letter mark",
        '\u{200B}' => "zero width space",
        '\u{200C}' => "zero width non-joiner",
        '\u{200D}' => "zero width joiner",
        '\u{2060}' => "word joiner",
        '\u{FEFF}' => "zero width no-break space",
        '\u{00AD}' => "soft hyphen",
        '\u{00A0}' => "no-break space",
        '\u{202F}' => "narrow no-break space",
        '\u{2007}' => "figure space",
        _ => "invisible character",
    }
}

/// Where a position of the source lies.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Place {
    Literal,
    Comment,
    Code,
}

fn place_of<'t>(root: Node<'t>, start: usize, end: usize) -> (Place, Option<Node<'t>>) {
    let Some(n) = root.descendant_for_byte_range(start, end) else {
        return (Place::Code, None);
    };
    match kind(n) {
        "string_fragment"
        | "string_literal"
        | "character_literal"
        | "escape_sequence"
        | "multiline_string_fragment" => (Place::Literal, Some(n)),
        "line_comment" | "block_comment" => (Place::Comment, Some(n)),
        _ => (Place::Code, Some(n)),
    }
}

/// Bytes worth a look: backslashes (Unicode escapes), control bytes other than tab, line
/// breaks and form feed, DEL, and the bytes of non-ASCII characters.
static INTERESTING: [bool; 256] = {
    let mut t = [false; 256];
    let mut b = 0usize;
    while b < 256 {
        t[b] = b == b'\\' as usize
            || b >= 0x7f
            || (b < 0x20
                && b != b'\t' as usize
                && b != b'\n' as usize
                && b != b'\r' as usize
                && b != 0x0c);
        b += 1;
    }
    t
};

/// The character a `\uXXXX` escape starting at byte `i` denotes, with the escape's length.
fn unicode_escape(bytes: &[u8], i: usize) -> Option<(char, usize)> {
    // Eligible only when preceded by an even number of backslashes.
    let mut backslashes = 0;
    let mut j = i;
    while j > 0 && bytes[j - 1] == b'\\' {
        backslashes += 1;
        j -= 1;
    }
    if backslashes % 2 == 1 {
        return None;
    }
    let mut k = i + 1;
    while k < bytes.len() && bytes[k] == b'u' {
        k += 1;
    }
    if k == i + 1 {
        return None;
    }
    let hex = bytes.get(k..k + 4)?;
    let s = std::str::from_utf8(hex).ok()?;
    let v = u32::from_str_radix(s, 16).ok()?;
    Some((char::from_u32(v)?, k + 4 - i))
}

impl<'t> JavaRule<'t> for InvisibleCharacter {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["program"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let src = ctx.source();
        let bytes = src.as_bytes();
        let trailing_start = src.trim_end().len();
        // One finding per enclosing node (literal, comment or token).
        let mut reported: Vec<usize> = Vec::new();
        let mut next = 0usize;
        // Skip to the next byte worth a look: a backslash, a control byte, non-ASCII.
        while let Some(off) = bytes
            .get(next..)
            .and_then(|rest| rest.iter().position(|&b| INTERESTING[b as usize]))
        {
            let i = next + off;
            // `i` is a character boundary: only ASCII bytes and whole characters are skipped.
            let Some(c) = src.get(i..).and_then(|s| s.chars().next()) else {
                break;
            };
            next = i + c.len_utf8();
            if c == '\\' {
                if bytes.get(i + 1) != Some(&b'u') {
                    continue;
                }
                let Some((decoded, len)) = unicode_escape(bytes, i) else {
                    continue;
                };
                let (place, at) = place_of(node, i, i + len);
                let report = match place {
                    Place::Literal => false,
                    Place::Code => true,
                    // In a comment the translated character stays in the comment, unless it
                    // ends a line comment early.
                    Place::Comment => matches!(decoded, '\n' | '\r'),
                };
                if !report {
                    continue;
                }
                if let Some(at) = at {
                    if reported.contains(&at.id()) {
                        continue;
                    }
                    reported.push(at.id());
                }
                let msg = if place == Place::Comment {
                    "This Unicode escape in a comment is translated by the compiler: remove it"
                } else {
                    "Write this character directly: Unicode escapes outside literals are \
                     translated before the code is read and hide what it says"
                };
                ctx.emit_range(Span::new(i, i + len), msg);
                continue;
            }
            let plain_ascii = (c as u32) < 0x7f
                && ((c as u32) >= 0x20 || matches!(c, '\t' | '\n' | '\r' | '\u{c}'));
            if plain_ascii {
                continue;
            }
            let Some(class) = classify(c) else {
                continue;
            };
            if c == '\u{FEFF}' && i == 0 {
                continue;
            }
            // The end-of-file marker of old editors.
            if c == '\u{1A}' && i >= trailing_start.saturating_sub(1) {
                continue;
            }
            let (place, at) = place_of(node, i, i + c.len_utf8());
            if place == Place::Comment && class != Class::Bidi {
                continue;
            }
            if let Some(at) = at {
                if reported.contains(&at.id()) {
                    continue;
                }
                reported.push(at.id());
            }
            let code = c as u32;
            let msg = match class {
                Class::Bidi => format!(
                    "Remove this bidirectional control character (U+{code:04X}, {}): it changes \
                     how the surrounding code is displayed",
                    describe(c)
                ),
                Class::Invisible if place == Place::Literal => format!(
                    "Write this invisible character (U+{code:04X}, {}) as the escape \
                     `\\u{code:04X}`",
                    describe(c)
                ),
                Class::Invisible => format!(
                    "Remove this invisible character (U+{code:04X}, {}) from the code",
                    describe(c)
                ),
                Class::Control => format!(
                    "Replace this raw control character (U+{code:04X}) with an escape or remove \
                     it"
                ),
            };
            ctx.emit_range(Span::new(i, i + c.len_utf8()), msg);
        }
    }
}
