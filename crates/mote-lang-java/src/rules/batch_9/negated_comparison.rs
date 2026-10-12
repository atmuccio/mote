//! `java/negated-comparison` — `!(a == b)` or `!(i < n)` where the opposite operator reads
//! better.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["readability"],
    remediation_minutes: 1,
    description: r#"Detects a negated comparison that can be written with the opposite
operator: `!(a == b)` is `a != b`, `!(a != b)` is `a == b`, and — for integer operands —
`!(i < n)` is `i >= n` (likewise `<=`, `>`, `>=`).

**Why it matters.** The negation wrapped around the parentheses is one more step for the
reader, and it is easy to miss when scanning a condition. The direct operator states the
condition as it is.

**How to fix.** Drop the `!` and the parentheses, and use the opposite operator.

```java
// Noncompliant
if (!(count == 0)) { ... }
while (!(index >= size)) { ... }

// Compliant
if (count != 0) { ... }
while (index < size) { ... }
```

Ordering comparisons are only reported when both operands are proven integers (`int`,
`long`, `short`, `byte`, `char` or their wrappers): with `float` or `double`, `!(a < b)` is
also true when either value is NaN, so it is not the same as `a >= b`. Off by default: this
is a readability preference."#,
    ..RuleMeta::new(
        "java/negated-comparison",
        "java",
        "Negated comparison instead of the opposite operator",
    )
};

pub struct NegatedComparison;

fn opposite(op: &str) -> Option<&'static str> {
    Some(match op {
        "==" => "!=",
        "!=" => "==",
        "<" => ">=",
        "<=" => ">",
        ">" => "<=",
        ">=" => "<",
        _ => return None,
    })
}

fn is_integral<'t>(ctx: &Ctx<'t>, e: Node<'t>) -> bool {
    ctx.type_of(e).is_some_and(|t| {
        matches!(
            t.unboxed(),
            Some(Prim::Int | Prim::Long | Prim::Short | Prim::Byte | Prim::Char)
        )
    })
}

impl<'t> JavaRule<'t> for NegatedComparison {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["unary_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if syntax::operator_str(node) != "!" {
            return;
        }
        let Some(operand) = node.field(Field::Operand) else {
            return;
        };
        if kind(operand) != "parenthesized_expression" {
            return;
        }
        let inner = syntax::strip_parens(operand);
        if kind(inner) != "binary_expression" {
            return;
        }
        let op = syntax::operator_str(inner);
        let Some(opp) = opposite(op) else {
            return;
        };
        if !matches!(op, "==" | "!=") {
            let (Some(l), Some(r)) = (inner.field(Field::Left), inner.field(Field::Right)) else {
                return;
            };
            if !is_integral(ctx, l) || !is_integral(ctx, r) {
                return;
            }
        }
        ctx.emit(
            node,
            format!("Use `{opp}` instead of negating the `{op}` comparison"),
        );
    }
}
