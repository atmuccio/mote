//! `java/case-insensitive-equals` — `a.toLowerCase().equals(b.toLowerCase())` instead of
//! `a.equalsIgnoreCase(b)`.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["strings", "performance"],
    remediation_minutes: 2,
    description: r#"Detects two strings compared after converting both to the same case in the
default locale — `a.toLowerCase().equals(b.toLowerCase())`, the same with `toUpperCase`, or
`a.toLowerCase().compareTo(b.toLowerCase())` — and a string converted to lower (upper) case
compared with a literal that is already all lower (upper) case:
`s.toLowerCase().equals("yes")`.

**Why it matters.** `equalsIgnoreCase` and `compareToIgnoreCase` say what is meant and compare
character by character without building two new strings for every comparison. The
conversions also depend on the default locale when no `Locale` is given: in a Turkish locale
`"TITLE".toLowerCase()` contains a dotless `ı`, so the comparison fails where
`equalsIgnoreCase` succeeds.

**How to fix.** Compare the original strings with the ignore-case method.

```java
// Noncompliant
if (header.toLowerCase().equals(expected.toLowerCase())) { … }
if (answer.toLowerCase().equals("yes")) { … }

// Compliant
if (header.equalsIgnoreCase(expected)) { … }
if ("yes".equalsIgnoreCase(answer)) { … }
```

Only receivers the resolver proves to be `String` are considered. Not reported: one side
converted and the other not (the other may legitimately be mixed case), conversions to
different cases, conversions with an explicit `Locale` (other than `Locale.getDefault()`), on both
sides or on one side compared
with a literal (`s.toLowerCase(Locale.ROOT).equals("yes")`: whole-string case mapping, which
differs from the character-wise ignore-case methods for a few letters such as `ſ` or `ı`, is
then a deliberate choice), `toUpperCase` before `compareTo` (it orders `_` and the other characters between
`Z` and `a` differently from `compareToIgnoreCase`), literals that contain letters of both cases or non-ASCII letters (whose case
mapping differs between the two approaches), `Objects.equals` (null-safe, unlike
`equalsIgnoreCase` on a `null` receiver), and other comparisons such as `startsWith` or
`contains`."#,
    ..RuleMeta::new(
        "java/case-insensitive-equals",
        "java",
        "Case-converted strings compared with equals",
    )
};

pub struct CaseInsensitiveEquals;

/// For `x.toLowerCase(..)` / `x.toUpperCase(..)` on a `String`: the method name.
/// For `x.toLowerCase(..)` / `x.toUpperCase(..)` on a `String`: the method name and whether a
/// `Locale` argument is given.
fn case_conversion<'t>(ctx: &Ctx<'t>, expr: Node<'t>) -> Option<(&'t str, bool)> {
    let expr = syntax::strip_parens(expr);
    if kind(expr) != "method_invocation" {
        return None;
    }
    let name = ctx.call_name(expr)?;
    let n = syntax::argument_count(expr);
    if !matches!(name, "toLowerCase" | "toUpperCase") || n > 1 {
        return None;
    }
    let recv = syntax::strip_parens(expr.field(Field::Object)?);
    let is_string =
        kind(recv) == "string_literal" || ctx.type_of(recv).is_some_and(|t| t.is_string());
    // `Locale.getDefault()` spells out the default locale: no deliberate choice.
    let explicit = syntax::arguments(expr)
        .next()
        .is_some_and(|a| !ctx.text(a).starts_with("Locale.getDefault("));
    is_string.then_some((name, explicit))
}

/// Whether `expr` is a plain string literal with at least one letter, all ASCII, and every
/// letter already in the case `conversion` produces.
fn literal_in_case(ctx: &Ctx<'_>, expr: Node<'_>, conversion: &str) -> bool {
    let expr = syntax::strip_parens(expr);
    if kind(expr) != "string_literal" || ctx.text(expr).starts_with("\"\"\"") {
        return false;
    }
    let Some(content) = syntax::string_literal_content(expr, ctx.source()) else {
        return false;
    };
    if !content.is_ascii() || content.contains('\\') {
        return false;
    }
    let mut letters = content
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .peekable();
    if letters.peek().is_none() {
        return false;
    }
    if conversion == "toLowerCase" {
        letters.all(|c| c.is_ascii_lowercase())
    } else {
        letters.all(|c| c.is_ascii_uppercase())
    }
}

impl<'t> JavaRule<'t> for CaseInsensitiveEquals {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["equals", "compareTo"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) = ctx.call_name(node) else {
            return;
        };
        if !matches!(name, "equals" | "compareTo") || syntax::argument_count(node) != 1 {
            return;
        }
        let (Some(recv), Some(arg)) = (node.field(Field::Object), syntax::arguments(node).next())
        else {
            return;
        };
        // Cheap text test before any resolver work.
        let recv_text = ctx.text(recv);
        let arg_text = ctx.text(arg);
        if !(recv_text.contains("Case(") || arg_text.contains("Case(")) {
            return;
        }
        let recv_conv = case_conversion(ctx, recv);
        let arg_conv = case_conversion(ctx, arg);
        let conv = match (recv_conv, arg_conv) {
            // Both sides converted in the default locale. With an explicit locale the author
            // chose whole-string case mapping, which differs from the character-wise ignore-case
            // methods for a few letters; `toUpperCase` before `compareTo` orders `_` and the
            // other characters between `Z` and `a` differently from `compareToIgnoreCase`.
            (Some((a, false)), Some((b, false)))
                if a == b && (name == "equals" || a == "toLowerCase") =>
            {
                a
            }
            (Some((a, false)), None) if name == "equals" && literal_in_case(ctx, arg, a) => a,
            (None, Some((b, false))) if name == "equals" && literal_in_case(ctx, recv, b) => b,
            _ => return,
        };
        let replacement = if name == "equals" {
            "equalsIgnoreCase"
        } else {
            "compareToIgnoreCase"
        };
        ctx.emit(
            node,
            format!("Use `{replacement}` instead of comparing strings converted with `{conv}`"),
        );
    }
}
