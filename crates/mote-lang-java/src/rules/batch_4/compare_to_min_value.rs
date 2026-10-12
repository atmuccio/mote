//! `java/compare-to-min-value` — `compareTo`/`compare` returning `Integer.MIN_VALUE`.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["comparison"],
    remediation_minutes: 2,
    description: r#"Detects a `compareTo(..)` or `compare(..)` implementation that returns
`Integer.MIN_VALUE`.

**Why it matters.** Callers commonly reverse an order by negating the result
(`-a.compareTo(b)`, `Comparator.reversed()` in hand-written code). In two's complement
`-Integer.MIN_VALUE` is still `Integer.MIN_VALUE`, so the "reversed" comparison still says
"less than" and sorting, binary search, and tree maps quietly misbehave.

**How to fix.** Return `-1`, `0`, or `1` (or delegate to `Integer.compare`).

```java
// Noncompliant
public int compareTo(Version other) {
    if (other == null) return Integer.MIN_VALUE;
    return Integer.compare(number, other.number);
}

// Compliant
public int compareTo(Version other) {
    if (other == null) return -1;
    return Integer.compare(number, other.number);
}
```

Only `return` statements of methods named `compareTo` (one parameter) or `compare` (two
parameters) are checked, including both branches of a returned conditional expression.
Returns inside lambdas are not considered."#,
    ..RuleMeta::new(
        "java/compare-to-min-value",
        "java",
        "Comparison method returns Integer.MIN_VALUE",
    )
};

pub struct CompareToMinValue;

/// The `Integer.MIN_VALUE` sub-expressions a returned expression can evaluate to.
fn min_value_results<'t>(ctx: &Ctx<'t>, expr: Node<'t>, out: &mut Vec<Node<'t>>, depth: u32) {
    if depth > 16 {
        return;
    }
    let expr = syntax::strip_parens(expr);
    match kind(expr) {
        "ternary_expression" => {
            for f in [F::Consequence, F::Alternative] {
                if let Some(b) = util::fld(expr, f) {
                    min_value_results(ctx, b, out, depth + 1);
                }
            }
        }
        "field_access" => {
            let (Some(obj), Some(field)) = (util::fld(expr, F::Object), util::fld(expr, F::Field))
            else {
                return;
            };
            if ctx.text(field) == "MIN_VALUE" && util::names_type(ctx, obj, "java.lang.Integer") {
                out.push(expr);
            }
        }
        "identifier" if ctx.text(expr) == "MIN_VALUE" && ctx.declaration_of(expr).is_none() => {
            let imported = ctx.imports().iter().any(|i| {
                i.is_static
                    && ((!i.is_wildcard && i.path == "java.lang.Integer.MIN_VALUE")
                        || (i.is_wildcard && i.path == "java.lang.Integer"))
            });
            if imported {
                out.push(expr);
            }
        }
        _ => {}
    }
}

impl<'t> JavaRule<'t> for CompareToMinValue {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["return_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !ctx.text(node).contains("MIN_VALUE") {
            return;
        }
        let Some(method) = ctx.enclosing_function(node) else {
            return;
        };
        if kind(method) != "method_declaration" {
            return;
        }
        let expected_params = match util::name_text(ctx, method) {
            "compareTo" => 1,
            "compare" => 2,
            _ => return,
        };
        if syntax::parameters(method).count() != expected_params
            || util::fld(method, F::Type).is_none_or(|t| ctx.text(t) != "int")
        {
            return;
        }
        let Some(value) = syntax::first_named_child(node) else {
            return;
        };
        let mut hits = Vec::new();
        min_value_results(ctx, value, &mut hits, 0);
        for hit in hits {
            ctx.emit(
                hit,
                "Return a small negative number such as -1 instead of Integer.MIN_VALUE, which stays negative when negated",
            );
        }
    }
}
