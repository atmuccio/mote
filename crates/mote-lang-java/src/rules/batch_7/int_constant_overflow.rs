//! `java/int-constant-overflow` — constant `int` arithmetic that overflows
//! (`24 * 60 * 60 * 1000 * 365`).

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["numbers", "overflow"],
    remediation_minutes: 2,
    description: r#"Detects arithmetic on `int` literals (`*`, `+`, `-`) whose exact result does not
fit in an `int`: `24 * 60 * 60 * 1000 * 365`, `3 * 1024 * 1024 * 1024`.

**Why it matters.** All operands are `int`, so the computation is done in 32 bits and wraps
around before the result is stored — even when it is assigned to a `long`. A year in
milliseconds becomes `1471228928`; three gigabytes becomes a negative number. The compiler
does not warn.

**How to fix.** Make the first operand a `long` literal so the whole product is computed in
64 bits, or use a unit API.

```java
// Noncompliant
long yearMillis = 365 * 24 * 60 * 60 * 1000;

// Compliant
long yearMillis = 365L * 24 * 60 * 60 * 1000;
long same = TimeUnit.DAYS.toMillis(365);
```

Only expressions made entirely of decimal `int` literals are checked (hexadecimal, octal and
binary literals are often deliberate bit patterns), and the smallest overflowing
sub-expression is reported."#,
    ..RuleMeta::new(
        "java/int-constant-overflow",
        "java",
        "Constant int arithmetic overflows",
    )
};

pub struct IntConstantOverflow;

/// The exact value of an expression made only of decimal `int` literals, `+`, `-`, `*`,
/// parentheses and unary minus; `None` for anything else.
fn eval(ctx: &Ctx<'_>, node: Node<'_>, depth: u32) -> Option<i128> {
    if depth > 64 {
        return None;
    }
    let node = syntax::strip_parens(node);
    match kind(node) {
        "decimal_integer_literal" => {
            let text = ctx.text(node);
            if text.ends_with(['l', 'L']) {
                return None;
            }
            let clean: String = text.chars().filter(|c| *c != '_').collect();
            clean.parse::<i128>().ok()
        }
        "unary_expression" => {
            let v = eval(ctx, node.field(Field::Operand)?, depth + 1)?;
            match syntax::operator_str(node) {
                "-" => Some(-v),
                "+" => Some(v),
                _ => None,
            }
        }
        "binary_expression" => {
            let op = syntax::operator_str(node);
            if !matches!(op, "*" | "+" | "-") {
                return None;
            }
            let l = eval(ctx, node.field(Field::Left)?, depth + 1)?;
            let r = eval(ctx, node.field(Field::Right)?, depth + 1)?;
            match op {
                "*" => l.checked_mul(r),
                "+" => l.checked_add(r),
                _ => l.checked_sub(r),
            }
        }
        _ => None,
    }
}

fn fits(v: i128) -> bool {
    (i32::MIN as i128..=i32::MAX as i128).contains(&v)
}

impl<'t> JavaRule<'t> for IntConstantOverflow {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["binary_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(right) = node.field(Field::Right) else {
            return;
        };
        if !matches!(
            kind(right),
            "decimal_integer_literal" | "parenthesized_expression"
        ) {
            return;
        }
        let Some(left) = node.field(Field::Left) else {
            return;
        };
        if !matches!(
            kind(left),
            "decimal_integer_literal" | "binary_expression" | "parenthesized_expression"
        ) {
            return;
        }
        if !matches!(ctx.operator_str(node), "*" | "+" | "-") {
            return;
        }
        let Some(v) = eval(ctx, node, 0) else {
            return;
        };
        if fits(v) {
            return;
        }
        // Report the innermost overflowing expression only.
        let child_overflows = |c: Node<'t>| eval(ctx, c, 0).is_some_and(|cv| !fits(cv));
        if child_overflows(left) || child_overflows(right) {
            return;
        }
        let wrapped = v as i32;
        ctx.emit(
            node,
            format!("Use a long operand (e.g. `L` suffix): this int arithmetic overflows to {wrapped} (the exact value is {v})"),
        );
    }
}
