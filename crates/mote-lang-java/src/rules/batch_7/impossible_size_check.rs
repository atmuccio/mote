//! `java/impossible-size-check` — `list.size() < 0`, `s.length() >= 0`, `a.length == -1`:
//! comparisons whose result is fixed because sizes are never negative.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Stable,
    tags: &["collections", "suspicious"],
    remediation_minutes: 2,
    description: r#"Detects comparisons of a size or length with a constant whose result never
changes because a size is never negative: `list.size() < 0` (always false),
`name.length() >= 0` (always true), `array.length == -1`, `map.size() > -1`.

**Why it matters.** The condition doesn't test anything. Usually the author meant a
different check — emptiness (`isEmpty()`, `> 0`), a bound (`< max`), or an `indexOf` result
compared with `-1` — and the guarded code now runs always or never.

**How to fix.** Write the comparison that was intended, or remove the dead condition.

```java
// Noncompliant
if (items.size() >= 0) { process(items.get(0)); }

// Compliant
if (items.size() > 0) { process(items.get(0)); }
```

Only `size()` of a `Collection` or `Map`, `length()` of a `CharSequence` (`String`,
`StringBuilder`, …) and `length` of an array are checked, when the resolver proves the
receiver's type; custom `size()` methods may return -1 and are not reported."#,
    ..RuleMeta::new(
        "java/impossible-size-check",
        "java",
        "Size compared with a value it can never have",
    )
};

pub struct ImpossibleSizeCheck;

/// Whether `expr` is a size/length the resolver proves non-negative.
fn is_size<'t>(ctx: &Ctx<'t>, expr: Node<'t>) -> bool {
    let expr = syntax::strip_parens(expr);
    match kind(expr) {
        "method_invocation" => {
            let Some(name) = ctx.call_name(expr) else {
                return false;
            };
            if syntax::argument_count(expr) != 0 {
                return false;
            }
            match name {
                "size" => util::receiver_is(ctx, expr, &["java.util.Collection", "java.util.Map"]),
                "length" => util::receiver_is(ctx, expr, &["java.lang.CharSequence"]),
                _ => false,
            }
        }
        "field_access" => {
            let (Some(o), Some(f)) = (expr.field(Field::Object), expr.field(Field::Field)) else {
                return false;
            };
            ctx.text(f) == "length" && ctx.type_of(o).is_some_and(|t| t.is_array())
        }
        _ => false,
    }
}

impl<'t> JavaRule<'t> for ImpossibleSizeCheck {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["binary_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(op) = ctx.operator(node) else {
            return;
        };
        let op = kind(op);
        if !matches!(op, "<" | "<=" | ">" | ">=" | "==" | "!=") {
            return;
        }
        let (Some(left), Some(right)) = (node.field(Field::Left), node.field(Field::Right)) else {
            return;
        };
        // Normalize to `size OP k`.
        let (size, k, op) = if let Some(k) = util::int_value(ctx, right) {
            (left, k, op)
        } else if let Some(k) = util::int_value(ctx, left) {
            let flipped = match op {
                "<" => ">",
                "<=" => ">=",
                ">" => "<",
                ">=" => "<=",
                o => o,
            };
            (right, k, flipped)
        } else {
            return;
        };
        let result = match op {
            "<" if k <= 0 => false,
            ">=" if k <= 0 => true,
            "<=" if k < 0 => false,
            ">" if k < 0 => true,
            "==" if k < 0 => false,
            "!=" if k < 0 => true,
            _ => return,
        };
        if !is_size(ctx, size) {
            return;
        }
        ctx.emit(
            node,
            format!(
                "Fix this condition: it is always {result} because `{}` is never negative",
                ctx.text(syntax::strip_parens(size))
            ),
        );
    }
}
