//! `js/no-misleading-character-class`

use super::regex;
use crate::prelude::*;
use oxc_regular_expression::ast::{CharacterClass, CharacterClassContents};

pub struct NoMisleadingCharacterClass;

pub static NO_MISLEADING_CHARACTER_CLASS: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["regex", "suspicious"],
    remediation_minutes: 5,
    description: r#"Flags a character class (`[…]`) that lists a character made of several code points as if it
were one: an emoji or other character outside the Basic Multilingual Plane without the `u` flag,
a letter followed by a combining accent, an emoji with a skin-tone modifier or variation selector,
a flag (two regional indicators), or an emoji sequence joined with zero-width joiners.

**Why:** a character class matches exactly one code point (one UTF-16 unit without the `u`
flag). Listing a multi-part character spreads its parts over the class: `/^[👍]$/` without `u`
matches half of the emoji and never the whole, `[é]` written as `e` + accent also matches a bare
accent, and `[🇫🇷]` matches either half of any flag.

**Fix:** add the `u` (or `v`) flag for characters outside the BMP; for sequences, use an
alternation instead of a class (`(?:👍🏽|👍)`), or with the `v` flag `[\q{👍🏽}]`.

```js
/^[👍]$/.test(s)        // flagged: two code units without the u flag
/[é]/                   // flagged when written as e + U+0301 (combining acute)
/^[👍]$/u.test(s)       // compliant
/^(?:👍🏽|👍)$/u         // compliant
```

Ranges (such as the common `[\u0300-\u036f]` for stripping accents) and marks listed without a
base character are not reported. Only regular expression literals are checked.
"#,
    ..RuleMeta::new(
        "js/no-misleading-character-class",
        "js",
        "Character classes should not contain characters made of several code points",
    )
};

impl JsRule for NoMisleadingCharacterClass {
    fn meta(&self) -> &'static RuleMeta {
        &NO_MISLEADING_CHARACTER_CLASS
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::RegExpLiteral]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::RegExpLiteral(lit) = node.kind() else {
            return;
        };
        let text = lit.regex.pattern.text.as_str();
        // Every problem needs a non-ASCII character or a `\u` escape inside a class.
        if !text.contains('[') || (text.is_ascii() && !text.contains("\\u")) {
            return;
        }
        let unicode = regex::is_unicode(lit);
        let allocator = oxc_allocator::Allocator::default();
        let Some(pattern) = regex::parse(&allocator, ctx, lit) else {
            return;
        };
        let mut found: Vec<(Span, &'static str)> = Vec::new();
        regex::for_each_class(&pattern, &mut |class| {
            if let Some(problem) = misleading(class, unicode) {
                found.push((class.span, problem));
            }
        });
        for (span, problem) in found {
            let span = super::super::batch_6::util::first_line(ctx, span);
            ctx.emit(
                span,
                format!("This character class lists {problem}, whose parts it matches one by one; use an alternation or the 'u' flag instead"),
            );
        }
    }
}

/// The first multi-code-point character listed in the class, described for the message.
fn misleading(class: &CharacterClass<'_>, unicode: bool) -> Option<&'static str> {
    let items: Vec<Option<u32>> = class
        .body
        .iter()
        .map(|item| match item {
            CharacterClassContents::Character(c) => Some(c.value),
            _ => None,
        })
        .collect();
    for (i, pair) in items.windows(2).enumerate() {
        let [Some(prev), Some(cur)] = *pair else {
            continue;
        };
        if !unicode && is_high_surrogate(prev) && is_low_surrogate(cur) {
            return Some("a character outside the Basic Multilingual Plane (two UTF-16 units without the 'u' flag)");
        }
        if is_base(prev) && is_combining_mark(cur) {
            return Some("a character with a combining mark");
        }
        if is_base(prev) && is_variation_selector(cur) {
            return Some("a character with a variation selector");
        }
        if prev >= 0x2000 && is_base(prev) && is_emoji_modifier(cur) {
            return Some("an emoji with a skin-tone modifier");
        }
        if is_regional_indicator(prev) && is_regional_indicator(cur) {
            return Some("a flag (a pair of regional indicators)");
        }
        if cur == 0x200D && prev >= 0x2000 && is_base(prev) {
            let next = items.get(i + 2).copied().flatten();
            if next.is_some_and(|n| n >= 0x2000 && is_base(n)) {
                return Some("an emoji sequence joined with a zero-width joiner");
            }
        }
    }
    None
}

fn is_high_surrogate(c: u32) -> bool {
    (0xD800..=0xDBFF).contains(&c)
}

fn is_low_surrogate(c: u32) -> bool {
    (0xDC00..=0xDFFF).contains(&c)
}

fn is_combining_mark(c: u32) -> bool {
    matches!(c, 0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F)
}

fn is_variation_selector(c: u32) -> bool {
    matches!(c, 0xFE00..=0xFE0F | 0xE0100..=0xE01EF)
}

fn is_emoji_modifier(c: u32) -> bool {
    (0x1F3FB..=0x1F3FF).contains(&c)
}

fn is_regional_indicator(c: u32) -> bool {
    (0x1F1E6..=0x1F1FF).contains(&c)
}

/// A character that can carry a following mark: not itself a mark, selector, modifier, joiner,
/// surrogate half, or ASCII punctuation/control.
fn is_base(c: u32) -> bool {
    let ascii_word = c < 0x80 && char::from_u32(c).is_some_and(|ch| ch.is_ascii_alphanumeric());
    (ascii_word || c > 0x7F)
        && !is_combining_mark(c)
        && !is_variation_selector(c)
        && !is_emoji_modifier(c)
        && !is_high_surrogate(c)
        && !is_low_surrogate(c)
        && !matches!(c, 0x200B..=0x200F | 0x2060..=0x206F | 0xFEFF)
}
