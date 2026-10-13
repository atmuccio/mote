//! `java/equals-without-type-check` — `equals(Object o)` casts `o` without checking its type.

use super::util::{self, Walk, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["equality"],
    remediation_minutes: 5,
    description: r#"Detects an `equals(Object)` implementation that casts its parameter without
any `instanceof` test or `getClass()` comparison.

**Why it matters.** The `equals` contract requires returning `false` for objects of other
types. A bare cast throws `ClassCastException` instead, so putting the object in a mixed
collection, comparing it with a proxy, or calling `list.contains("x")` blows up far away
from the actual mistake.

**How to fix.** Check the type first — `instanceof` (with a pattern variable) or
`getClass() != o.getClass()` when subclasses must never be equal.

```java
// Noncompliant
@Override
public boolean equals(Object o) {
    Point p = (Point) o;
    return x == p.x && y == p.y;
}

// Compliant
@Override
public boolean equals(Object o) {
    return o instanceof Point p && x == p.x && y == p.y;
}
```

Not reported when the method contains any `instanceof`, type pattern, `getClass()`,
`isInstance`/`isAssignableFrom` call, a `catch` clause, or passes the parameter to another
method first (for example `super.equals(o)` or a helper that may check the type)."#,
    ..RuleMeta::new(
        "java/equals-without-type-check",
        "java",
        "equals() casts its argument without a type check",
    )
};

pub struct EqualsWithoutTypeCheck;

impl<'t> JavaRule<'t> for EqualsWithoutTypeCheck {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_declaration"]
    }

    fn method_names(&self) -> &'static [&'static str] {
        &["equals"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if util::name_text(ctx, node) != "equals" {
            return;
        }
        let mut params = syntax::parameters(node);
        let (Some(param), None) = (params.next(), params.next()) else {
            return;
        };
        // `equals(Object... o)` does not override `Object.equals`.
        let Some(ptype) = util::fld(param, F::Type).filter(|_| kind(param) == "formal_parameter")
        else {
            return;
        };
        if !matches!(ctx.text(ptype), "Object" | "java.lang.Object") {
            return;
        }
        let Some(pname) = util::fld(param, F::Name).map(|n| ctx.text(n)) else {
            return;
        };
        let Some(body) = util::fld(node, F::Body) else {
            return;
        };
        let is_param = |n: Node<'t>| {
            let n = syntax::strip_parens(n);
            kind(n) == "identifier" && ctx.text(n) == pname
        };
        let mut guarded = false;
        let mut cast: Option<Node<'t>> = None;
        util::visit(body, |n| match kind(n) {
            "instanceof_expression" | "type_pattern" | "record_pattern" | "catch_clause" => {
                guarded = true;
                Walk::Stop
            }
            "method_invocation" => {
                let name = util::name_text(ctx, n);
                if matches!(
                    name,
                    "getClass" | "isInstance" | "isAssignableFrom" | "cast"
                ) || util::args(n).any(is_param)
                {
                    guarded = true;
                    Walk::Stop
                } else {
                    Walk::Continue
                }
            }
            // `new Helper(o)` may check the type as well.
            "object_creation_expression" => {
                if util::args(n).any(is_param) {
                    guarded = true;
                    Walk::Stop
                } else {
                    Walk::Continue
                }
            }
            "cast_expression" => {
                if cast.is_none() && util::fld(n, F::Value).is_some_and(is_param) {
                    cast = Some(n);
                }
                Walk::Continue
            }
            "lambda_expression" | "class_body" => Walk::Skip,
            _ => Walk::Continue,
        });
        if guarded {
            return;
        }
        let Some(cast) = cast else {
            return;
        };
        ctx.emit(
            cast,
            format!("Check the type of `{pname}` (instanceof or getClass()) before casting it: equals() must return false for other types, not throw ClassCastException"),
        );
    }
}
