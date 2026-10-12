//! `js/no-multi-str` (off by default; part of the `conventions` set)

use crate::prelude::*;

pub struct NoMultiStr;

pub static NO_MULTI_STR: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Incubating,
    default_enabled: false,
    tags: &["conventions"],
    remediation_minutes: 2,
    description: r#"Flags string literals continued on the next line with a backslash at the end of the line.

**Why:** a backslash before a line break removes the line break from the string, but nothing
else: the next line's indentation becomes part of the text, and a single space after the
backslash turns the continuation into a syntax error that is invisible in most editors. The
string's real content is hard to see.

**Fix:** join the pieces with `+`, use a template literal (whose line breaks are kept), or
build the text with `join`.

```js
const message = 'The file could not \
  be saved';                                   // flagged: the text contains the spaces
const message = 'The file could not ' +
  'be saved';                                  // compliant
const message = `The file could not
be saved`;                                     // compliant: the line break is part of the text
```

Not reported: template literals, and string attributes in JSX, where a backslash is an ordinary
character.

Off by default: part of the opt-in `conventions` set.
"#,
    ..RuleMeta::new(
        "js/no-multi-str",
        "js",
        "String literals should not be continued with a backslash line break",
    )
};

impl JsRule for NoMultiStr {
    fn meta(&self) -> &'static RuleMeta {
        &NO_MULTI_STR
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::StringLiteral]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::StringLiteral(lit) = node.kind() else {
            return;
        };
        // A continuation makes the source longer than the value plus its quotes: most literals
        // are rejected here without reading their text.
        if (lit.span.size() as usize) <= lit.value.len() + 2 {
            return;
        }
        let raw = ctx.text(lit.span);
        // A raw line terminator inside a quoted literal can only follow a backslash (anything
        // else is a syntax error); U+2028/U+2029 may appear unescaped, so check those for a
        // preceding backslash.
        let continued = raw.bytes().any(|b| b == b'\n' || b == b'\r')
            || raw.contains("\\\u{2028}")
            || raw.contains("\\\u{2029}");
        if !continued {
            return;
        }
        if matches!(ctx.parent_kind(node), Some(AstKind::JSXAttribute(_))) {
            return;
        }
        if !matches!(raw.as_bytes().first(), Some(b'"' | b'\'')) {
            return;
        }
        ctx.emit(
            Span::new(lit.span.start, lit.span.start + 1),
            "Replace the backslash line continuation with concatenation or a template literal",
        );
    }
}
