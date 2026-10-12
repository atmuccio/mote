//! `java/collection-size-comparison` — a resolver-based rule.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["collections", "readability"],
    remediation_minutes: 1,
    description: r#"Detects emptiness checks written as size comparisons: `x.size() == 0`,
`x.size() != 0`, `x.size() > 0`, `x.size() < 1`, `x.size() >= 1`, `x.size() <= 0` (and the
mirrored forms) on collections and maps, and the same comparisons of `length()` on strings.

**Why it matters.** `isEmpty()` states the intent directly and is never slower; for some
collections (for example concurrent ones) computing the size is expensive while checking
emptiness is constant time.

**How to fix.** Use `isEmpty()` or `!isEmpty()`.

```java
// Noncompliant
if (orders.size() == 0) { return; }
if (name.length() > 0) { greet(name); }

// Compliant
if (orders.isEmpty()) { return; }
if (!name.isEmpty()) { greet(name); }
```

Only receivers whose type is known to be a `Collection`, a `Map`, or a `String` are reported."#,
    ..RuleMeta::new(
        "java/collection-size-comparison",
        "java",
        "Emptiness checked by comparing the size",
    )
};

pub struct CollectionSizeComparison;

/// `Some(true)` = the comparison means "is empty", `Some(false)` = "is not empty".
fn emptiness(op: &str, value: i64, call_on_left: bool) -> Option<bool> {
    // Normalise to `call OP value`.
    let op = if call_on_left {
        op
    } else {
        match op {
            "<" => ">",
            ">" => "<",
            "<=" => ">=",
            ">=" => "<=",
            other => other,
        }
    };
    match (op, value) {
        ("==", 0) | ("<", 1) | ("<=", 0) => Some(true),
        ("!=", 0) | (">", 0) | (">=", 1) => Some(false),
        _ => None,
    }
}

impl<'t> JavaRule<'t> for CollectionSizeComparison {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["binary_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let op = ctx.operator_str(node);
        if !matches!(op, "==" | "!=" | "<" | ">" | "<=" | ">=") {
            return;
        }
        let (Some(left), Some(right)) = (node.field(Field::Left), node.field(Field::Right)) else {
            return;
        };
        let (call, lit, call_on_left) = if kind(syntax::strip_parens(left)) == "method_invocation" {
            (syntax::strip_parens(left), right, true)
        } else if kind(syntax::strip_parens(right)) == "method_invocation" {
            (syntax::strip_parens(right), left, false)
        } else {
            return;
        };
        let Some(value) = syntax::int_literal_value(lit, ctx.source()) else {
            return;
        };
        let Some(is_empty) = emptiness(op, value, call_on_left) else {
            return;
        };
        let Some(name) = call.field(Field::Name) else {
            return;
        };
        let method = ctx.text(name);
        if method != "size" && method != "length" {
            return;
        }
        if syntax::argument_count(call) != 0 {
            return;
        }
        let Some(recv) = call.field(Field::Object) else {
            return;
        };
        if matches!(kind(recv), "this" | "super") {
            return;
        }
        // Don't suggest isEmpty() inside an isEmpty() implementation.
        if ctx
            .enclosing_method(node)
            .and_then(|m| m.field(Field::Name))
            .is_some_and(|n| ctx.text(n) == "isEmpty")
        {
            return;
        }
        let Some(t) = ctx.type_of(recv) else { return };
        let what = if method == "size" {
            if ctx.is_subtype_of(&t, "java.util.Collection") {
                "collection"
            } else if ctx.is_subtype_of(&t, "java.util.Map") {
                "map"
            } else {
                return;
            }
        } else if t.is_string() {
            "string"
        } else {
            return;
        };
        let msg = if is_empty {
            format!("Use isEmpty() to check whether this {what} is empty")
        } else {
            format!("Use !isEmpty() to check whether this {what} is non-empty")
        };
        ctx.emit(node, msg);
    }
}

#[cfg(test)]
mod tests {
    use super::emptiness;

    #[test]
    fn normalises_operand_order() {
        assert_eq!(emptiness("==", 0, true), Some(true));
        assert_eq!(emptiness(">", 0, true), Some(false));
        assert_eq!(emptiness("<", 0, false), Some(false)); // 0 < x.size()
        assert_eq!(emptiness(">", 1, false), Some(true)); // 1 > x.size()
        assert_eq!(emptiness(">", 1, true), None); // x.size() > 1 is not an emptiness check
    }
}
