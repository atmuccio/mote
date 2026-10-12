//! `java/compare-to-specific-value` — `a.compareTo(b) == 1`.

use super::util::{self, Fld, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["comparison"],
    remediation_minutes: 2,
    description: r#"Detects the result of `compareTo(..)` or `compare(..)` being tested for equality
with `1` or `-1` (`== 1`, `== -1`, `!= 1`, `!= -1`).

**Why it matters.** The comparison contract only promises a negative number, zero, or a
positive number. Many implementations return other values — `String.compareTo` returns the
difference between the first differing characters, and hand-written comparators often return
`a - b` — so `compareTo(..) == 1` is `false` for many "greater" results.

**How to fix.** Test the sign: `> 0`, `< 0`, `>= 0`, `<= 0`.

```java
// Noncompliant
if (version.compareTo(required) == -1) { upgrade(); }

// Compliant
if (version.compareTo(required) < 0) { upgrade(); }
```

Only calls named `compareTo` with one argument or `compare` with two arguments are
considered."#,
    ..RuleMeta::new(
        "java/compare-to-specific-value",
        "java",
        "compareTo() result compared with a specific value",
    )
};

pub struct CompareToSpecificValue;

fn is_comparison_call<'t>(ctx: &Ctx<'t>, n: Node<'t>) -> bool {
    let n = syntax::strip_parens(n);
    if kind(n) != "method_invocation" {
        return false;
    }
    match util::method_name(ctx, n) {
        "compareTo" => util::arg_count(n) == 1,
        "compare" => util::arg_count(n) == 2,
        _ => false,
    }
}

impl<'t> JavaRule<'t> for CompareToSpecificValue {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["binary_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let op = ctx.operator_str(node);
        if op != "==" && op != "!=" {
            return;
        }
        let (Some(left), Some(right)) = (node.fld(F::Left), node.fld(F::Right)) else {
            return;
        };
        // Cheap shape test first: a call on one side, `1` / `-1` on the other.
        let lit = if is_comparison_call(ctx, left) {
            right
        } else if is_comparison_call(ctx, right) {
            left
        } else {
            return;
        };
        let Some(value) = syntax::int_literal_value(lit, ctx.source()) else {
            return;
        };
        if value != 1 && value != -1 {
            return;
        }
        let sign = match (op, value) {
            ("==", 1) => "> 0",
            ("==", _) => "< 0",
            ("!=", 1) => "<= 0",
            _ => ">= 0",
        };
        let at = util::op(node).unwrap_or(node);
        ctx.emit(
            at,
            format!("Test the sign of the comparison result (`{sign}`) instead of `{op} {value}`"),
        );
    }
}
