//! `java/substring-then-search` — `s.substring(k).startsWith(p)` / `.indexOf(x)`, where the
//! overload taking a start offset avoids copying the string.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Incubating,
    tags: &["performance", "strings"],
    remediation_minutes: 5,
    description: r#"Detects a `String` search applied to the result of `substring(k)`:
`s.substring(k).startsWith(p)` and `s.substring(k).indexOf(x)`.

**Why it matters.** `substring` copies the characters from `k` to the end into a new string,
only to search it once and drop it. `String` has overloads that search from an offset without
copying anything: `s.startsWith(p, k)` and `s.indexOf(x, k)`. In a loop over a long text the
copies turn a linear scan into a quadratic one and produce garbage for the collector.

**How to fix.** Search the original string from the offset. `indexOf(x, k)` returns a
position in the original string: subtract `k` if the position within the substring was
needed, and keep testing for `-1` before subtracting.

```java
// Noncompliant
if (line.substring(pos).startsWith("--")) { … }
int end = line.substring(pos).indexOf(';');

// Compliant
if (line.startsWith("--", pos)) { … }
int at = line.indexOf(';', pos);
int end = at < 0 ? -1 : at - pos;
```

Only `substring` with one argument on an expression the resolver proves to be a `String` is
considered. Not reported: `substring(begin, end)` (the searches have no end bound before
Java 21), `lastIndexOf` (its offset overload searches backwards from the offset, which is
not the same), `contains`, `endsWith` and `equals`, which have no offset overload."#,
    ..RuleMeta::new(
        "java/substring-then-search",
        "java",
        "Search on a substring instead of an offset",
    )
};

pub struct SubstringThenSearch;

impl<'t> JavaRule<'t> for SubstringThenSearch {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["startsWith", "indexOf"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) = ctx.call_name(node) else {
            return;
        };
        if !matches!(name, "startsWith" | "indexOf") || syntax::argument_count(node) != 1 {
            return;
        }
        let Some(sub) = node.field(Field::Object).map(syntax::strip_parens) else {
            return;
        };
        if kind(sub) != "method_invocation"
            || ctx.call_name(sub) != Some("substring")
            || syntax::argument_count(sub) != 1
        {
            return;
        }
        if !util::receiver_is_string(ctx, sub) {
            return;
        }
        let (Some(at), Some(k)) = (sub.field(Field::Name), util::arg(sub, 0)) else {
            return;
        };
        let offset = ctx.text(k);
        let offset = if offset.len() <= 20 && !offset.contains('\n') {
            offset
        } else {
            "k"
        };
        ctx.emit_range(
            Span::new(at.start_byte(), node.end_byte()),
            format!(
                "Call `{name}(.., {offset})` on the original string instead of copying it with `substring({offset})` first"
            ),
        );
    }
}
