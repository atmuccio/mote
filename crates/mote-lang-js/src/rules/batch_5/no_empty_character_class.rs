//! `js/no-empty-character-class`

use crate::prelude::*;

pub struct NoEmptyCharacterClass;

pub static NO_EMPTY_CHARACTER_CLASS: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["regex", "suspicious"],
    remediation_minutes: 5,
    description: r#"Flags an empty character class `[]` in a regular expression literal.

**Why:** `[]` is valid syntax but matches no character at all, so any regular expression that
requires it can never match: `/^a[]b/.test(s)` is always `false`. It usually comes from an
unescaped `]` (`/[]a]/` was meant to match `]` or `a`) or from a class whose contents were deleted.

**Fix:** put the intended characters in the class, escape a literal bracket (`/[\]a]/`), or use
`[^]` / `[\s\S]` if "any character" was meant.

```js
const tag = /<[]+>/;       // flagged: can never match
const tag = /<[^>]+>/;     // compliant
const any = /[^]/;         // compliant: the negated empty class matches any character
```
"#,
    ..RuleMeta::new(
        "js/no-empty-character-class",
        "js",
        "Regular expressions should not contain empty character classes",
    )
};

impl JsRule for NoEmptyCharacterClass {
    fn meta(&self) -> &'static RuleMeta {
        &NO_EMPTY_CHARACTER_CLASS
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::RegExpLiteral]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::RegExpLiteral(lit) = node.kind() else {
            return;
        };
        let pattern = lit.regex.pattern.text.as_str();
        if !pattern.contains("[]") {
            return;
        }
        let nested = lit.regex.flags.contains(RegExpFlags::V);
        let Some(offset) = empty_class_offset(pattern, nested) else {
            return;
        };
        // The pattern starts right after the opening `/`.
        let start = lit.span.start + 1 + offset as u32;
        ctx.emit(
            Span::new(start, start + 2),
            "Remove or fill this empty character class: '[]' matches nothing, so the expression can never match",
        );
    }
}

/// Byte offset of the first `[]` that opens an empty class (not escaped, not inside another class
/// unless the `v` flag allows nested classes).
fn empty_class_offset(pattern: &str, nested: bool) -> Option<usize> {
    let bytes = pattern.as_bytes();
    let mut depth = 0usize;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 1,
            b'[' if depth == 0 || nested => {
                if bytes.get(i + 1) == Some(&b']') {
                    return Some(i);
                }
                depth += 1;
            }
            b']' if depth > 0 => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    None
}
