//! `java/invalid-regex-literal` — a string-literal regular expression that cannot compile.

use super::regex;
use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["regex"],
    remediation_minutes: 5,
    description: r#"Detects a string literal passed as a regular expression — to
`Pattern.compile`, `Pattern.matches`, or `String.matches`, `replaceAll`, `replaceFirst`,
`split` — that is not valid `java.util.regex` syntax: unbalanced parentheses, an unclosed
character class, a quantifier with nothing to repeat (`*abc`, `a|+b`), a `{` that does not
start a repetition (`"{name}"`, `"${user}"`), a reversed range (`[z-a]`), or a trailing
lone backslash.

**Why it matters.** The pattern is only compiled at run time, so the mistake surfaces as a
`PatternSyntaxException` on the first execution — often on an error path or a rarely used
feature. A frequent case is treating `replaceAll` / `split` as plain-text operations:
`s.replaceAll("{name}", value)` always throws.

**How to fix.** Escape the metacharacters (`"\\{name\\}"`), quote the text with
`Pattern.quote(..)`, or use `String.replace` for literal replacements.

```java
// Noncompliant
template.replaceAll("${user}", name);
line.split("*");

// Compliant
template.replace("${user}", name);
line.split("\\*");
```

The checker is conservative: patterns using constructs it does not model (`\Q…\E`, nested
or intersected classes, comments mode, flags passed to `compile`) are never reported."#,
    ..RuleMeta::new(
        "java/invalid-regex-literal",
        "java",
        "Invalid regular expression literal",
    )
};

#[derive(Default)]
pub struct InvalidRegexLiteral {
    regex_calls: util::Gate,
}

impl<'t> JavaRule<'t> for InvalidRegexLiteral {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["compile", "matches", "replaceAll", "replaceFirst", "split"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !self.regex_calls.open_any(
            ctx,
            &[
                "compile(",
                "matches(",
                "replaceAll(",
                "replaceFirst(",
                "split(",
            ],
        ) {
            return;
        }
        let Some(name_node) = util::fld(node, F::Name) else {
            return;
        };
        let name = ctx.text(name_node);
        if !matches!(
            name,
            "compile" | "matches" | "replaceAll" | "replaceFirst" | "split"
        ) {
            return;
        }
        let n = util::arg_count(node);
        let static_pattern = match (name, n) {
            ("compile", 1) | ("matches", 2) => true,
            ("matches", 1) | ("replaceAll" | "replaceFirst", 2) | ("split", 1 | 2) => false,
            _ => return,
        };
        let Some(arg) = util::first_arg(node) else {
            return;
        };
        if !syntax::is_string_literal(arg) {
            return;
        }
        // Every reported mistake involves one of these characters.
        if !ctx
            .text(arg)
            .contains(['(', ')', '[', ']', '{', '}', '*', '+', '?', '\\'])
        {
            return;
        }
        let Some(recv) = util::fld(node, F::Object) else {
            return;
        };
        let Some(pattern) = util::string_value(ctx, arg) else {
            return;
        };
        let Some(reason) = regex::definite_error(&pattern) else {
            return;
        };
        let receiver_ok = if static_pattern {
            util::names_type(ctx, recv, "java.util.regex.Pattern")
        } else {
            kind(recv) == "string_literal" || ctx.type_of(recv).is_some_and(|t| t.is_string())
        };
        if !receiver_ok {
            return;
        }
        ctx.emit(
            arg,
            format!("Fix this regular expression: {reason}, so it throws PatternSyntaxException at run time"),
        );
    }
}
