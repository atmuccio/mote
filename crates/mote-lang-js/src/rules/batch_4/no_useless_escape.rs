//! `js/no-useless-escape`

use crate::prelude::*;

pub struct NoUselessEscape;

pub static NO_USELESS_ESCAPE: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["redundant", "regex"],
    remediation_minutes: 1,
    description: r#"Flags backslashes that do not change the meaning of the next character in strings, template
literals, and regular expression literals: `'\:'`, `` `\"` ``, `/\#/`, `/[\.]/`.

**Why:** an unnecessary escape adds noise and suggests that the character is special when it is
not. It is also often a sign of a misunderstanding — for example `'\d'` in a string is just `d`
(the author probably meant `'\\d'` for a regular expression), and `/\e/` matches the letter `e`
rather than an escape character.

**Fix:** remove the backslash, or double it if a literal backslash was intended.

```js
const time = '12\:30';          // flagged
const quote = "it\'s";          // flagged: no need to escape ' inside "…"
const price = /\$\d+\,\d{2}/;   // flagged: the comma needs no escape
const dots = /[\.\*]/;          // flagged (twice): no need inside a character class

const time = '12:30';           // compliant
const quote = 'it\'s';          // compliant: the delimiter must be escaped
const price = /\$\d+,\d{2}/;    // compliant: `$` and `\d` need the backslash
```

Not checked: tagged templates (the tag sees the raw text, as with `String.raw`), JSX attribute
strings (backslashes are not escapes there), and regular expressions with the `u` or `v` flag
(where an unnecessary escape is already a syntax error, or required by the syntax). Inside
character classes, `\-`, `\^`, `\[`, `\]`, `\/`, and `\\` are never reported.
"#,
    ..RuleMeta::new(
        "js/no-useless-escape",
        "js",
        "Unnecessary escape characters should be removed",
    )
};

impl JsRule for NoUselessEscape {
    fn meta(&self) -> &'static RuleMeta {
        &NO_USELESS_ESCAPE
    }

    fn types(&self) -> &'static [AstType] {
        &[
            AstType::StringLiteral,
            AstType::TemplateLiteral,
            AstType::RegExpLiteral,
        ]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let mut found: Vec<(u32, char)> = Vec::new();
        match node.kind() {
            AstKind::StringLiteral(s) => {
                let text = ctx.text(s.span);
                if !text.contains('\\') {
                    return;
                }
                if matches!(ctx.parent_kind(node), Some(AstKind::JSXAttribute(_))) {
                    return;
                }
                let Some(quote) = text.chars().next().filter(|c| *c == '"' || *c == '\'') else {
                    return;
                };
                scan_string(text, quote, &mut |at, c| found.push((s.span.start + at, c)));
            }
            AstKind::TemplateLiteral(t) => {
                if matches!(
                    ctx.parent_kind(node),
                    Some(AstKind::TaggedTemplateExpression(_))
                ) {
                    return;
                }
                for quasi in &t.quasis {
                    let text = ctx.text(quasi.span);
                    if !text.contains('\\') || text != quasi.value.raw.as_str() {
                        continue;
                    }
                    let start = quasi.span.start;
                    scan_template(text, &mut |at, c| found.push((start + at, c)));
                }
            }
            AstKind::RegExpLiteral(r) => {
                if r.regex.flags.intersects(RegExpFlags::U | RegExpFlags::V) {
                    return;
                }
                let text = ctx.text(r.span);
                // `/pattern/flags`: the pattern is between the first and the last slash.
                let (Some(open), Some(close)) = (text.find('/'), text.rfind('/')) else {
                    return;
                };
                if open != 0 || close <= open {
                    return;
                }
                let Some(pattern) = text.get(1..close) else {
                    return;
                };
                if !pattern.contains('\\') {
                    return;
                }
                scan_regex(pattern, &mut |at, c| found.push((r.span.start + 1 + at, c)));
            }
            _ => return,
        }
        for (at, c) in found {
            let span = Span::new(at, at + 1 + c.len_utf8() as u32);
            ctx.emit(
                span,
                format!("Remove the unnecessary backslash before '{c}'"),
            );
        }
    }
}

fn is_line_terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// Escapes that mean something in both string and template literals.
fn is_string_escape(c: char) -> bool {
    matches!(c, '\\' | 'n' | 'r' | 'v' | 't' | 'b' | 'f' | 'u' | 'x')
        || c.is_ascii_digit()
        || is_line_terminator(c)
}

/// Call `report(offset_of_backslash, escaped_char)` for each useless escape in a quoted string.
fn scan_string(text: &str, quote: char, report: &mut dyn FnMut(u32, char)) {
    let mut chars = text.char_indices().skip(1).peekable();
    while let Some((i, c)) = chars.next() {
        if c != '\\' {
            continue;
        }
        let Some((_, next)) = chars.next() else {
            break;
        };
        if !is_string_escape(next) && next != quote {
            report(i as u32, next);
        }
    }
}

/// Like [`scan_string`] for the raw text of one template element.
fn scan_template(text: &str, report: &mut dyn FnMut(u32, char)) {
    let bytes = text.as_bytes();
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c != '\\' {
            continue;
        }
        let Some((j, next)) = chars.next() else {
            break;
        };
        let needed = is_string_escape(next)
            || next == '`'
            // `\${` prevents a substitution; `\$` elsewhere does nothing.
            || (next == '$' && bytes.get(j + 1) == Some(&b'{'))
            // `$\{` also prevents a substitution.
            || (next == '{' && i > 0 && bytes.get(i - 1) == Some(&b'$'));
        if !needed {
            report(i as u32, next);
        }
    }
}

/// Useless escapes in a regular expression pattern without the `u`/`v` flags.
fn scan_regex(pattern: &str, report: &mut dyn FnMut(u32, char)) {
    let mut in_class = false;
    let mut chars = pattern.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' => {
                let Some((_, next)) = chars.next() else {
                    break;
                };
                let needed = if in_class {
                    needed_in_class(next)
                } else {
                    needed_outside_class(next)
                };
                if !needed {
                    report(i as u32, next);
                }
            }
            '[' if !in_class => in_class = true,
            ']' if in_class => in_class = false,
            _ => {}
        }
    }
}

/// Letters and digits with a meaning after a backslash (character classes, assertions, control,
/// hex, unicode, back-references, named back-references, property escapes).
fn is_regex_escape_letter(c: char) -> bool {
    matches!(
        c,
        'b' | 'B'
            | 'c'
            | 'd'
            | 'D'
            | 'f'
            | 'k'
            | 'n'
            | 'p'
            | 'P'
            | 'r'
            | 's'
            | 'S'
            | 't'
            | 'v'
            | 'w'
            | 'W'
            | 'x'
            | 'u'
    ) || c.is_ascii_digit()
}

fn needed_outside_class(c: char) -> bool {
    is_regex_escape_letter(c)
        || matches!(
            c,
            '\\' | '/'
                | '^'
                | '$'
                | '.'
                | '*'
                | '+'
                | '?'
                | '('
                | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | '|'
        )
}

fn needed_in_class(c: char) -> bool {
    // `\B` and `\k` have no meaning inside a class, but are left alone like the other letters
    // that are escapes outside of one. `\/` and `\[` are widespread habits (and required with the
    // `v` flag), so they are not reported either.
    is_regex_escape_letter(c) || matches!(c, '\\' | ']' | '-' | '^' | '/' | '[')
}
