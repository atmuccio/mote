//! `js/invalid-replacement-reference`

use crate::prelude::*;

pub struct InvalidReplacementReference;

pub static INVALID_REPLACEMENT_REFERENCE: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["regex", "suspicious"],
    remediation_minutes: 5,
    description: r#"Flags replacement strings of `replace` / `replaceAll` that refer to a capture group the regular
expression does not have: `$3` when the pattern has two groups, `$1` when it has none, or
`$<name>` when no group has that name.

**Why:** a reference to a missing group is not an error. `$3` is inserted as the literal text
`$3`, and a missing `$<name>` is replaced by nothing, so the output is silently wrong. It usually
means a group was removed from the pattern (or turned into a non-capturing `(?:…)`) without
updating the replacement, or the groups were miscounted.

**Fix:** refer to an existing group, add the missing group to the pattern, or write `$$` for a
literal dollar sign.

```js
date.replace(/(\d+)-(\d+)/, '$2/$3');        // flagged: there is no group 3
text.replace(/(?:a|b)+/g, '[$1]');          // flagged: the group is non-capturing
text.replace(/(?<word>\w+)/, '<$<wrod>>');  // flagged: no group named 'wrod'

date.replace(/(\d+)-(\d+)/, '$2/$1');        // compliant
price.replace(/\d+/, '$$&');                // compliant: `$$` is a dollar sign
```

Only a regular expression literal combined with a string literal (or a template literal without
substitutions) is checked.
"#,
    ..RuleMeta::new(
        "js/invalid-replacement-reference",
        "js",
        "Replacement strings should only refer to existing capture groups",
    )
};

impl JsRule for InvalidReplacementReference {
    fn meta(&self) -> &'static RuleMeta {
        &INVALID_REPLACEMENT_REFERENCE
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CallExpression]
    }

    fn callee_names(&self) -> &'static [&'static str] {
        &["replace", "replaceAll"]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::CallExpression(call) = node.kind() else {
            return;
        };
        let Some(member) = call.callee.as_member_expression() else {
            return;
        };
        if !matches!(
            member.static_property_name(),
            Some("replace" | "replaceAll")
        ) || call.arguments.len() != 2
        {
            return;
        }
        let (Some(pattern), Some(replacement)) = (
            call.arguments.first().and_then(Argument::as_expression),
            call.arguments.get(1).and_then(Argument::as_expression),
        ) else {
            return;
        };
        let Expression::RegExpLiteral(regex) = helpers::unwrap_expr(pattern) else {
            return;
        };
        let replacement = helpers::unwrap_expr(replacement);
        let Some(text) = helpers::string_value(replacement) else {
            return;
        };
        if !text.contains('$') {
            return;
        }
        let unicode_sets = regex.regex.flags.contains(RegExpFlags::V);
        let Some(groups) = capture_groups(regex.regex.pattern.text.as_str(), unicode_sets) else {
            return;
        };
        let Some(problem) = first_invalid_reference(text, &groups) else {
            return;
        };
        let message = match problem {
            Invalid::Index(reference) if groups.count == 0 => format!(
                "Remove '{reference}' or add a capture group: the regular expression has no capturing groups, so it is inserted literally"
            ),
            Invalid::Index(reference) => format!(
                "Fix '{reference}': the regular expression has only {} capturing group{}, so it is inserted literally",
                groups.count,
                if groups.count == 1 { "" } else { "s" }
            ),
            Invalid::Name(name) => format!(
                "Fix '$<{name}>': the regular expression has no group named '{name}', so it is replaced by nothing"
            ),
        };
        ctx.emit(replacement.span(), message);
    }
}

struct Groups<'a> {
    count: usize,
    names: Vec<&'a str>,
}

enum Invalid<'a> {
    Index(&'a str),
    Name(&'a str),
}

/// Count the capturing groups of a pattern and collect their names. `None` when the pattern looks
/// malformed (unbalanced class), to stay silent.
fn capture_groups(pattern: &str, unicode_sets: bool) -> Option<Groups<'_>> {
    let bytes = pattern.as_bytes();
    let mut groups = Groups {
        count: 0,
        names: Vec::new(),
    };
    let mut class_depth = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => {
                i += 2;
                continue;
            }
            b'[' => {
                if class_depth == 0 || unicode_sets {
                    class_depth += 1;
                }
            }
            b']' if class_depth > 0 => class_depth -= 1,
            b'(' if class_depth == 0 => {
                let rest = &pattern[i + 1..];
                if !rest.starts_with('?') {
                    groups.count += 1;
                } else if let Some(after) = rest.strip_prefix("?<") {
                    if !after.starts_with('=') && !after.starts_with('!') {
                        groups.count += 1;
                        let end = after.find('>')?;
                        groups.names.push(&after[..end]);
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    (class_depth == 0).then_some(groups)
}

/// The first `$n`, `$nn`, or `$<name>` of the replacement that does not match a group.
fn first_invalid_reference<'a>(text: &'a str, groups: &Groups<'_>) -> Option<Invalid<'a>> {
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while i + 1 < bytes.len() {
        if bytes[i] != b'$' {
            i += 1;
            continue;
        }
        let next = bytes[i + 1];
        match next {
            b'$' | b'&' | b'`' | b'\'' => i += 2,
            b'1'..=b'9' => {
                let d1 = usize::from(next - b'0');
                let two = bytes
                    .get(i + 2)
                    .filter(|b| b.is_ascii_digit())
                    .map(|b| d1 * 10 + usize::from(b - b'0'));
                let valid = two.is_some_and(|n| n <= groups.count) || d1 <= groups.count;
                if !valid {
                    return Some(Invalid::Index(&text[i..i + 2]));
                }
                i += if two.is_some_and(|n| n <= groups.count) {
                    3
                } else {
                    2
                };
            }
            b'<' if !groups.names.is_empty() => {
                let end = text[i + 2..].find('>')?;
                let name = &text[i + 2..i + 2 + end];
                if !groups.names.contains(&name) {
                    return Some(Invalid::Name(name));
                }
                i += 3 + end;
            }
            _ => i += 1,
        }
    }
    None
}
