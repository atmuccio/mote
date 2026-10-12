//! `java/string-call-with-known-result` — a `String` method call whose result is known without
//! running it.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["strings", "redundant-code"],
    remediation_minutes: 2,
    description: r#"Detects calls on a `String` whose result is fixed by their arguments:

* `replace(x, x)` — the same literal, character or variable on both sides; likewise
  `replaceAll` / `replaceFirst` with the same plain text (no regex or `$` syntax) twice;
* `substring(0)` and `s.substring(0, s.length())`, `concat("")`, `repeat(1)` — the string
  itself;
* `startsWith("")`, `endsWith("")`, `contains("")` — always `true`.

**Why it matters.** Such a call does nothing, so it usually stands for a call with the
wrong arguments: `path.replace("\\", "\\")` meant `replace("\\", "/")`, an escape sequence
lost a backslash, or a prefix constant that is empty by mistake makes a check always pass.
When it really is a no-op, it is noise the reader has to verify.

**How to fix.** Pass the intended arguments, or remove the call.

```java
// Noncompliant
String unix = path.replace("\\", "\\");
if (name.startsWith("")) { ... }

// Compliant
String unix = path.replace("\\", "/");
if (name.startsWith(prefix)) { ... }
```

Only receivers the resolver proves to be `String` are reported, and only when the result is
used (a discarded call is left to `java/ignored-return-value`). `toString()` on a `String`
is covered by `java/redundant-tostring`; arguments that are method calls (which could
return different values) are never compared."#,
    ..RuleMeta::new(
        "java/string-call-with-known-result",
        "java",
        "String call with a known result",
    )
};

pub struct StringCallWithKnownResult;

/// Characters with a meaning in a regex or in a replacement string.
const REGEX_SYNTAX: &[char] = &[
    '\\', '^', '$', '.', '|', '?', '*', '+', '(', ')', '[', ']', '{', '}',
];

/// The source text of an argument when it is a literal or a plain variable reference.
fn simple_arg<'t>(ctx: &Ctx<'t>, arg: Node<'t>) -> Option<&'t str> {
    let a = syntax::strip_parens(arg);
    match kind(a) {
        "string_literal" | "character_literal" | "identifier" => Some(ctx.text(a)),
        "field_access" => {
            let obj = a.field(Field::Object)?;
            matches!(kind(obj), "this" | "identifier").then(|| ctx.text(a))
        }
        _ => None,
    }
}

fn is_empty_string(ctx: &Ctx<'_>, arg: Node<'_>) -> bool {
    let a = syntax::strip_parens(arg);
    kind(a) == "string_literal" && ctx.text(a) == "\"\""
}

impl<'t> JavaRule<'t> for StringCallWithKnownResult {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &[
            "replace",
            "replaceAll",
            "replaceFirst",
            "substring",
            "concat",
            "repeat",
            "startsWith",
            "endsWith",
            "contains",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) = ctx.call_name(node) else {
            return;
        };
        let n = util::arg_count(node);
        let Some(a0) = util::arg(node, 0) else {
            return;
        };
        let msg: String = match (name, n) {
            ("replace", 2) => {
                let (Some(x), Some(y)) = (
                    simple_arg(ctx, a0),
                    util::arg(node, 1).and_then(|a| simple_arg(ctx, a)),
                ) else {
                    return;
                };
                if x != y {
                    return;
                }
                format!("Fix the arguments of this `replace`: it replaces {x} with itself, so the string is returned unchanged")
            }
            ("replaceAll" | "replaceFirst", 2) => {
                let a0 = syntax::strip_parens(a0);
                let Some(a1) = util::arg(node, 1).map(syntax::strip_parens) else {
                    return;
                };
                if kind(a0) != "string_literal" || kind(a1) != "string_literal" {
                    return;
                }
                let (x, y) = (ctx.text(a0), ctx.text(a1));
                if x != y || x.starts_with("\"\"\"") || x.contains(REGEX_SYNTAX) {
                    return;
                }
                format!("Fix the arguments of this `{name}`: it replaces {x} with itself, so the string is returned unchanged")
            }
            ("substring", 1) => {
                if syntax::int_literal_value(a0, ctx.source()) != Some(0) {
                    return;
                }
                "Remove this `substring(0)`: it returns the whole string".to_string()
            }
            ("substring", 2) => {
                if syntax::int_literal_value(a0, ctx.source()) != Some(0) {
                    return;
                }
                let Some(end) = util::arg(node, 1).map(syntax::strip_parens) else {
                    return;
                };
                let Some(recv) = node.field(Field::Object) else {
                    return;
                };
                let same_length = kind(end) == "method_invocation"
                    && ctx.call_name(end) == Some("length")
                    && util::arg_count(end) == 0
                    && end
                        .field(Field::Object)
                        .is_some_and(|o| simple_arg(ctx, o).is_some_and(|t| t == ctx.text(recv)))
                    && simple_arg(ctx, recv).is_some();
                if !same_length {
                    return;
                }
                "Remove this `substring(0, length())`: it returns the whole string".to_string()
            }
            ("concat", 1) if is_empty_string(ctx, a0) => {
                "Remove this `concat(\"\")`: it returns the string unchanged".to_string()
            }
            ("repeat", 1) if syntax::int_literal_value(a0, ctx.source()) == Some(1) => {
                "Remove this `repeat(1)`: it returns the string unchanged".to_string()
            }
            ("startsWith" | "endsWith" | "contains", 1) if is_empty_string(ctx, a0) => {
                format!("Fix this check: `{name}(\"\")` is always true")
            }
            _ => return,
        };
        // A discarded result: the call is there for its side effect (a null check), and
        // `java/ignored-return-value` covers the rest.
        if ctx
            .parent(node)
            .is_some_and(|p| kind(p) == "expression_statement")
        {
            return;
        }
        if !util::receiver_is_string(ctx, node) {
            return;
        }
        let at = node.field(Field::Name).unwrap_or(node);
        ctx.emit(at, msg);
    }
}
