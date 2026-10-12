//! `js/invisible-character`

use crate::prelude::*;

pub struct InvisibleCharacter;

pub static INVISIBLE_CHARACTER: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["unicode", "confusing", "security"],
    remediation_minutes: 5,
    description: r#"Flags characters in the source that a reader cannot see but the engine does:

* bidirectional control characters (U+202A–U+202E, U+2066–U+2069, U+200E, U+200F, U+061C)
  anywhere, comments included: they reorder how the text after them is displayed, so code can
  look different from what runs;
* raw zero-width characters (zero-width space and joiners, word joiner, a byte order mark inside
  the file, Hangul fillers) and raw control characters in strings, template literals, regular
  expressions and code (identifiers, the space between tokens);
* no-break spaces (U+00A0, U+202F, U+2007) and soft hyphens in code, and in literals where they
  pass for something else: on their own (a one-character string), at the start or end of the
  literal, or next to an ordinary space.

**Why:** two strings that print the same but differ by an invisible character compare unequal,
and a search for one does not find the other; an identifier holding a zero-width joiner is a
different identifier from the one it looks like; a no-break space between tokens looks like an
ordinary space; a bidirectional override can make a comment seem to end where it does not, or a
string seem to hold what it does not. None of this shows in a code review.

**Fix:** remove the character, or write it as an escape (`'\u{A0}'`), which states the intent.
In JSX text and attribute values, where escapes are not read, use an expression
(`{'\u{200B}'}`) or an HTML entity.

```js
const price = amount + ' EUR';       // flagged if this space is a raw no-break space (U+00A0)
const price = amount + '\u{A0}EUR';  // compliant
```

Not reported: escapes (`'\u{200B}'`), a byte order mark at the start of the file; a no-break
space or soft hyphen between two visible characters of a literal (`64 KB`, French spacing inside
`« »` or before `:`, a hyphenation point inside a long word), which is typography in text meant
for display, and every no-break space or soft hyphen in JSX text and attribute values; zero-width
joiners and non-joiners next to other non-ASCII characters (they join emoji into one picture, such
as a family of three, and shape Persian or Indic words); invisible characters other than
bidirectional controls in comments; and C1 control characters (U+0080 to U+009F), which appear
when a file in a legacy encoding is read as Latin-1: such files are skipped. JavaScript does not
translate `\u` escapes outside literals before parsing (unlike Java), so an escape in a comment
is just text and is not reported.
"#,
    ..RuleMeta::new(
        "js/invisible-character",
        "js",
        "Source code should not contain invisible or bidirectional control characters",
    )
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    /// Reorders the display of the text around it.
    Bidi,
    /// Takes no space at all.
    ZeroWidth,
    /// Looks like an ordinary space or hyphen but is not one.
    Spacing,
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
        | '\u{180E}'
        | '\u{3164}'
        | '\u{115F}'
        | '\u{1160}'
        | '\u{FFA0}' => Some(Class::ZeroWidth),
        '\u{00A0}' | '\u{202F}' | '\u{2007}' | '\u{00AD}' => Some(Class::Spacing),
        '\u{0}'..='\u{8}' | '\u{B}' | '\u{E}'..='\u{1F}' | '\u{7F}' => Some(Class::Control),
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

/// Is the byte worth a look: a control byte other than tab (0x09), line feed (0x0A), form feed
/// (0x0C) and carriage return (0x0D), DEL, or a byte of a non-ASCII character? Written without
/// short-circuits so that the block test below compiles to vector code.
#[inline(always)]
fn interesting(b: u8) -> bool {
    (b >= 0x7f) | ((b < 0x20) & (b != 0x09) & (b != 0x0a) & (b != 0x0c) & (b != 0x0d))
}

/// The position of the first interesting byte of `bytes`. Most source is plain ASCII: blocks of
/// 32 bytes are tested whole and skipped.
fn find_interesting(bytes: &[u8]) -> Option<usize> {
    const BLOCK: usize = 32;
    let mut skipped = 0;
    for block in bytes.chunks_exact(BLOCK) {
        let mut any = false;
        for &b in block {
            any |= interesting(b);
        }
        if any {
            break;
        }
        skipped += BLOCK;
    }
    let rest = bytes.get(skipped..)?;
    rest.iter()
        .position(|&b| interesting(b))
        .map(|p| skipped + p)
}

/// A character the rule may report, before its place in the file is known.
struct Hit {
    offset: u32,
    c: char,
    class: Class,
}

/// Calls `f` with every invisible, bidirectional or control character of the source (a byte
/// order mark at the start excepted) until it returns `false`. Returns `false` when `f` stopped
/// the scan or the scan met a C1 control character (a file in a legacy encoding read as Latin-1,
/// where any character may be an artefact of the decoding).
fn scan(src: &str, f: &mut dyn FnMut(Hit) -> bool) -> bool {
    let bytes = src.as_bytes();
    let mut next = 0usize;
    loop {
        let Some(off) = bytes.get(next..).and_then(find_interesting) else {
            return true;
        };
        let i = next + off;
        // `i` is a character boundary: only ASCII bytes and whole characters are skipped.
        let Some(c) = src.get(i..).and_then(|s| s.chars().next()) else {
            return true;
        };
        next = i + c.len_utf8();
        if ('\u{80}'..='\u{9F}').contains(&c) {
            return false;
        }
        let Some(class) = classify(c) else {
            continue;
        };
        if c == '\u{FEFF}' && i == 0 {
            continue;
        }
        // Joiners inside emoji sequences and words of other scripts.
        if matches!(c, '\u{200C}' | '\u{200D}') {
            let before = src.get(..i).and_then(|s| s.chars().next_back());
            let after = src.get(next..).and_then(|s| s.chars().next());
            if before.is_some_and(|b| !b.is_ascii()) || after.is_some_and(|a| !a.is_ascii()) {
                continue;
            }
        }
        let Ok(offset) = u32::try_from(i) else {
            return true;
        };
        if !f(Hit { offset, c, class }) {
            return false;
        }
    }
}

/// Where a character lies.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Place {
    /// A string, template or regular expression literal, where an escape can replace it.
    Literal,
    /// JSX text or a JSX attribute string: rendered text, where escapes are not read.
    Jsx,
    Comment,
    Code,
}

/// The literal spans of the file, collected during the traversal.
#[derive(Default)]
struct Literals {
    spans: Vec<(Span, Place)>,
}

impl JsRule for InvisibleCharacter {
    fn meta(&self) -> &'static RuleMeta {
        &INVISIBLE_CHARACTER
    }

    fn types(&self) -> &'static [AstType] {
        &[
            AstType::StringLiteral,
            AstType::TemplateElement,
            AstType::RegExpLiteral,
            AstType::JSXText,
        ]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        // Most files hold none of these characters: one byte scan, and the rule is skipped.
        let mut found = false;
        scan(ctx.source(), &mut |_| {
            found = true;
            false
        });
        found
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let span = node.span();
        // The content of the literal, without its delimiters.
        let (content, place) = match node.kind() {
            AstKind::StringLiteral(_) => {
                let content = Span::new(span.start + 1, span.end.saturating_sub(1).max(span.start));
                if matches!(ctx.parent_kind(node), Some(AstKind::JSXAttribute(_))) {
                    (content, Place::Jsx)
                } else {
                    (content, Place::Literal)
                }
            }
            AstKind::RegExpLiteral(lit) => {
                let len = lit.regex.pattern.text.len() as u32;
                (
                    Span::new(span.start + 1, span.start + 1 + len),
                    Place::Literal,
                )
            }
            AstKind::TemplateElement(_) => (span, Place::Literal),
            AstKind::JSXText(_) => (span, Place::Jsx),
            _ => return,
        };
        ctx.state::<Literals>().spans.push((content, place));
    }

    fn run_once<'a>(&self, ctx: &mut Ctx<'a>) {
        let mut literals = std::mem::take(&mut ctx.state::<Literals>().spans);
        literals.sort_by_key(|(span, _)| span.start);
        let comments = ctx.comments();
        let mut hits: Vec<Hit> = Vec::new();
        if !scan(ctx.source(), &mut |hit| {
            hits.push(hit);
            true
        }) {
            return;
        }
        // One finding per literal, comment, or line of code.
        let mut reported: Vec<(Place, u32)> = Vec::new();
        for Hit { offset, c, class } in hits {
            let (place, key, content) =
                if let Some(comment) = containing(comments, offset, |c| c.span) {
                    (Place::Comment, comment.span.start, comment.span)
                } else if let Some((span, place)) = containing(&literals, offset, |(s, _)| *s) {
                    (*place, span.start, *span)
                } else {
                    (Place::Code, ctx.line_of(offset), Span::default())
                };
            if place == Place::Comment && class != Class::Bidi {
                continue;
            }
            if class == Class::Spacing
                && (place == Place::Jsx
                    || (place == Place::Literal && between_visible(ctx, content, offset, c)))
            {
                continue;
            }
            if reported.contains(&(place, key)) {
                continue;
            }
            reported.push((place, key));
            let code = c as u32;
            let name = describe(c);
            let message = match (class, place) {
                (Class::Bidi, Place::Literal) => format!(
                    "Write this bidirectional control character (U+{code:04X}, {name}) as the escape '\\u{code:04X}': raw, it changes how the surrounding code is displayed"
                ),
                (Class::Bidi, _) => format!(
                    "Remove this bidirectional control character (U+{code:04X}, {name}): it changes how the surrounding code is displayed"
                ),
                (Class::Control, _) => format!(
                    "Replace this raw control character (U+{code:04X}) with an escape or remove it"
                ),
                (_, Place::Literal) => format!(
                    "Write this invisible character (U+{code:04X}, {name}) as the escape '\\u{code:04X}'"
                ),
                (_, Place::Jsx) => format!(
                    "Write this invisible character (U+{code:04X}, {name}) as {{'\\u{code:04X}'}} or an HTML entity, or remove it"
                ),
                (_, _) => format!(
                    "Remove this invisible character (U+{code:04X}, {name}) from the code"
                ),
            };
            ctx.emit(Span::new(offset, offset + c.len_utf8() as u32), message);
        }
    }
}

/// Is the character at `offset` between two visible characters of the same text (`10 kg`,
/// `« mot »`, `Donau-dampf` with a soft hyphen)? There a no-break space or soft hyphen is
/// typography: it keeps a value with its unit, or marks where a word may break. At the edge of a
/// literal, next to a space, or on its own it passes for an ordinary space or for nothing.
fn between_visible(ctx: &Ctx<'_>, content: Span, offset: u32, c: char) -> bool {
    let end = offset + c.len_utf8() as u32;
    if offset < content.start || content.end < end {
        return false;
    }
    let visible = |n: Option<char>| n.is_some_and(|n| !n.is_whitespace() && classify(n).is_none());
    visible(
        ctx.text(Span::new(content.start, offset))
            .chars()
            .next_back(),
    ) && visible(ctx.text(Span::new(end, content.end)).chars().next())
}

/// The item of `items` (sorted by start, spans disjoint) whose span contains `offset`.
fn containing<T>(items: &[T], offset: u32, span: impl Fn(&T) -> Span) -> Option<&T> {
    let i = items.partition_point(|item| span(item).start <= offset);
    let item = items.get(i.checked_sub(1)?)?;
    (offset < span(item).end).then_some(item)
}
