//! `java/separator-used-as-regex` — `split(".")`, `split("|")` and `File.separator` passed
//! where a regular expression is expected.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["regex", "strings", "portability"],
    remediation_minutes: 2,
    description: r#"Detects text meant literally that is passed to a method taking a regular
expression:

* `s.split(".")` and `s.split("|")` on a `String`;
* `File.separator` as the pattern of `split`, `replaceAll`, `replaceFirst`, `matches`, or
  `Pattern.compile`.

**Why it matters.** `String.split` takes a regex. `"."` matches every character, so
`"a.b".split(".")` returns an **empty** array; `"|"` is an empty alternation that matches
between all characters, so the string is cut into single characters. `File.separator` is
`"\"` on Windows — a dangling escape that makes the call throw `PatternSyntaxException`
there, while it works on Linux and macOS.

**How to fix.** Escape the character, or quote the text.

```java
// Noncompliant
String[] parts = version.split(".");
String[] dirs = path.split(File.separator);

// Compliant
String[] parts = version.split("\\.");
String[] dirs = path.split(Pattern.quote(File.separator));
```

Only `String` receivers the resolver proves are checked. `replaceAll(".", "*")` (masking
every character) is deliberate and not reported."#,
    ..RuleMeta::new(
        "java/separator-used-as-regex",
        "java",
        "Separator text passed as a regular expression",
    )
};

#[derive(Default)]
pub struct SeparatorUsedAsRegex;

/// Whether `expr` is `File.separator` (or `java.io.File.separator`).
fn is_file_separator<'t>(ctx: &Ctx<'t>, expr: Node<'t>) -> bool {
    let expr = syntax::strip_parens(expr);
    if kind(expr) != "field_access" {
        return false;
    }
    let (Some(o), Some(f)) = (expr.field(Field::Object), expr.field(Field::Field)) else {
        return false;
    };
    ctx.text(f) == "separator" && util::names_type(ctx, o, "java.io.File")
}

impl<'t> JavaRule<'t> for SeparatorUsedAsRegex {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["split", "replaceAll", "replaceFirst", "matches", "compile"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) = ctx.call_name(node) else {
            return;
        };
        let n = util::arg_count(node);
        let Some(pattern) = util::arg(node, 0) else {
            return;
        };
        let pattern = syntax::strip_parens(pattern);
        let is_literal = kind(pattern) == "string_literal";
        match (name, is_literal) {
            ("split", true) if n <= 2 => {
                let content = syntax::string_literal_content(pattern, ctx.source());
                let msg = match content {
                    Some(".") => "`split(\".\")` splits on every character and returns an empty array; use `\"\\\\.\"`",
                    Some("|") => "`split(\"|\")` splits between every character; use `\"\\\\|\"`",
                    _ => return,
                };
                if !receiver_is_string(ctx, node) {
                    return;
                }
                ctx.emit(pattern, format!("Escape this separator: {msg}"));
            }
            (_, false) => {
                if !is_file_separator(ctx, pattern) {
                    return;
                }
                let ok = match name {
                    "split" => n <= 2,
                    "replaceAll" | "replaceFirst" => n == 2,
                    "matches" => n == 1,
                    "compile" => {
                        n <= 2
                            && node.field(Field::Object).is_some_and(|o| {
                                util::names_type(ctx, o, "java.util.regex.Pattern")
                            })
                    }
                    _ => false,
                };
                if !ok || (name != "compile" && !receiver_is_string(ctx, node)) {
                    return;
                }
                ctx.emit(
                    pattern,
                    "Quote this pattern with `Pattern.quote(File.separator)`: on Windows the separator is a lone backslash, an invalid regular expression",
                );
            }
            _ => {}
        }
    }
}

fn receiver_is_string<'t>(ctx: &Ctx<'t>, call: Node<'t>) -> bool {
    call.field(Field::Object)
        .and_then(|o| ctx.type_of(o))
        .is_some_and(|t| t.is_string())
}
