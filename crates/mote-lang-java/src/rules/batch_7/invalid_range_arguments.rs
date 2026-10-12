//! `java/invalid-range-arguments` — `substring(5, 2)`, `subList(-1, n)`,
//! `Math.clamp(v, 10, 0)`: constant range bounds that always throw.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["api-misuse", "strings", "collections"],
    remediation_minutes: 2,
    description: r#"Detects range arguments written as constants that can never be valid:

* `substring`/`subSequence` on a string or builder, and `subList` on a list, with a negative
  bound or a start greater than the end (`s.substring(5, 2)`, `list.subList(-1, 3)`);
* `Arrays.copyOfRange(a, from, to)` with `from` negative or greater than `to`;
* `Math.clamp(value, min, max)` with `min` greater than `max`.

**Why it matters.** Each of these calls throws (`StringIndexOutOfBoundsException`,
`IndexOutOfBoundsException`, `IllegalArgumentException`) every time it runs. Swapped or
mistyped bounds are easy to miss when the code path is rare, such as error handling.

**How to fix.** Put the bounds in the right order and make them non-negative.

```java
// Noncompliant
String year = date.substring(4, 0);

// Compliant
String year = date.substring(0, 4);
```

Only integer literals are evaluated, and only receivers the resolver proves (`String`,
`CharSequence`, `List`, `java.util.Arrays`, `Math`)."#,
    ..RuleMeta::new(
        "java/invalid-range-arguments",
        "java",
        "Range bounds that always throw",
    )
};

pub struct InvalidRangeArguments;

impl<'t> JavaRule<'t> for InvalidRangeArguments {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &[
            "substring",
            "subSequence",
            "subList",
            "copyOfRange",
            "clamp",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) = ctx.call_name(node) else {
            return;
        };
        let n = util::arg_count(node);
        // (index of the first bound, whether a single bound is allowed)
        let first = match (name, n) {
            ("substring", 1) => 0,
            ("substring" | "subSequence" | "subList", 2) => 0,
            ("copyOfRange", 3) | ("clamp", 3) => 1,
            _ => return,
        };
        let lo_node = util::arg(node, first);
        let hi_node = if n > first + 1 {
            util::arg(node, first + 1)
        } else {
            None
        };
        let lo = lo_node.and_then(|a| util::int_value(ctx, a));
        let hi = hi_node.and_then(|a| util::int_value(ctx, a));
        let problem = if name == "clamp" {
            match (lo, hi) {
                (Some(a), Some(b)) if a > b => {
                    format!("the minimum {a} is greater than the maximum {b}")
                }
                _ => return,
            }
        } else {
            match (lo, hi) {
                (Some(a), _) if a < 0 => format!("the start {a} is negative"),
                (_, Some(b)) if b < 0 => format!("the end {b} is negative"),
                (Some(a), Some(b)) if a > b => format!("the start {a} is after the end {b}"),
                _ => return,
            }
        };
        let proven = match name {
            "substring" | "subSequence" => {
                util::receiver_is(ctx, node, &["java.lang.CharSequence"])
            }
            "subList" => util::receiver_is(ctx, node, &["java.util.List"]),
            "copyOfRange" => node
                .field(Field::Object)
                .is_some_and(|o| util::names_type(ctx, o, "java.util.Arrays")),
            _ => node
                .field(Field::Object)
                .is_some_and(|o| util::names_math(ctx, o)),
        };
        if !proven {
            return;
        }
        let at = lo_node.unwrap_or(node);
        ctx.emit(
            at,
            format!("Fix the bounds of `{name}`: {problem}, so this call always throws"),
        );
    }
}
