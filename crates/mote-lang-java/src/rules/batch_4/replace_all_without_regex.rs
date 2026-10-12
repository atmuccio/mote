//! `java/replace-all-without-regex` — `replaceAll` with a pattern that has no regex syntax.

use super::regex::META_CHARS;
use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["regex", "performance", "clarity"],
    remediation_minutes: 1,
    description: r#"Detects `String.replaceAll(pattern, replacement)` where the pattern is a string
literal without any regular-expression metacharacter and the replacement is a literal
without `$` or `\`.

**Why it matters.** `replaceAll` compiles its first argument as a regular expression on
every call and interprets `$` and `\` in the replacement. When the pattern is plain text,
`replace(..)` does exactly the same replacement (all occurrences) without the compilation
cost — and states the intent clearly. Readers of `replaceAll` must otherwise check whether
some character in the pattern is special.

**How to fix.** Call `replace(..)`.

```java
// Noncompliant
String csv = line.replaceAll(";", ",");

// Compliant
String csv = line.replace(";", ",");
```

Patterns containing any of `\ ^ $ . | ? * + ( ) [ ] { }`, empty patterns, and non-literal
replacements are not reported."#,
    ..RuleMeta::new(
        "java/replace-all-without-regex",
        "java",
        "replaceAll() with a plain-text pattern",
    )
};

#[derive(Default)]
pub struct ReplaceAllWithoutRegex {
    replace_all: util::Gate,
}

impl<'t> JavaRule<'t> for ReplaceAllWithoutRegex {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["replaceAll"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !self.replace_all.open(ctx, "replaceAll") {
            return;
        }
        let Some(name) = util::fld(node, F::Name) else {
            return;
        };
        if ctx.text(name) != "replaceAll" || util::arg_count(node) != 2 {
            return;
        }
        let mut args = util::args(node);
        let (Some(pattern), Some(replacement)) = (args.next(), args.next()) else {
            return;
        };
        let Some(p) = util::string_value(ctx, pattern) else {
            return;
        };
        if p.is_empty() || p.contains(META_CHARS) {
            return;
        }
        let Some(r) = util::string_value(ctx, replacement) else {
            return;
        };
        if r.contains(['$', '\\']) {
            return;
        }
        let Some(recv) = util::fld(node, F::Object) else {
            return;
        };
        if kind(recv) != "string_literal" && !ctx.type_of(recv).is_some_and(|t| t.is_string()) {
            return;
        }
        ctx.emit(
            name,
            "Use replace(..) instead of replaceAll(..): the pattern contains no regular-expression syntax",
        );
    }
}
