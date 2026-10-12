//! `java/unclear-operator-precedence` — operators whose relative precedence readers often get
//! wrong, mixed without parentheses.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["clarity", "operators"],
    remediation_minutes: 1,
    description: r#"Detects an operand that is itself an operation of a different precedence
level, written without parentheses, for the operator pairs that are commonly misread:

* `&&` inside `||` (`a && b || c`);
* arithmetic inside a shift (`x << n + 1`, `a + b >> 1`);
* one bitwise operator inside another (`a & b | c`, `a ^ b & c`);
* a comparison inside a bitwise operator (`flag & x == y`).

**Why it matters.** Everyone knows that `*` binds more tightly than `+`, but few readers are
sure whether `<<` binds more tightly than `+` (it does not: `1 << n + 1` shifts by `n + 1`),
whether `&` binds more tightly than `|`, or how `&&` and `||` group. Code that depends on these
rules is read wrongly, and a wrong reading is "fixed" into a bug. Parentheses cost nothing and
state the intended grouping.

**How to fix.** Add parentheses around the inner operation, after checking that the grouping
is the intended one.

```java
// Noncompliant
if (admin && active || override) { … }
int mask = 1 << bit + 1;

// Compliant
if ((admin && active) || override) { … }
int mask = 1 << (bit + 1);
```

The rule is off by default: mixing `&&` and `||` without parentheses is common in existing
code. Not reported: chains of one operator (`a || b || c`), arithmetic mixed with arithmetic
(`a + b * c`), comparisons inside `&&` / `||`, and shifts inside bitwise operators
(`hi << 8 | lo`)."#,
    ..RuleMeta::new(
        "java/unclear-operator-precedence",
        "java",
        "Operators of different precedence mixed without parentheses",
    )
};

pub struct UnclearOperatorPrecedence;

fn is_shift(op: &str) -> bool {
    matches!(op, "<<" | ">>" | ">>>")
}

fn is_arithmetic(op: &str) -> bool {
    matches!(op, "+" | "-" | "*" | "/" | "%")
}

fn is_bitwise(op: &str) -> bool {
    matches!(op, "&" | "|" | "^")
}

fn is_comparison(op: &str) -> bool {
    matches!(op, "==" | "!=" | "<" | ">" | "<=" | ">=")
}

/// Whether an unparenthesized `inner` operation as an operand of `outer` is unclear.
fn unclear(outer: &str, inner: &str) -> bool {
    (outer == "||" && inner == "&&")
        || (is_shift(outer) && is_arithmetic(inner))
        || (is_bitwise(outer) && is_bitwise(inner) && outer != inner)
        || (is_bitwise(outer) && is_comparison(inner))
}

impl<'t> JavaRule<'t> for UnclearOperatorPrecedence {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["binary_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let outer = ctx.operator_str(node);
        if !(outer == "||" || is_shift(outer) || is_bitwise(outer)) {
            return;
        }
        for side in [Field::Left, Field::Right] {
            let Some(child) = node.field(side) else {
                continue;
            };
            if kind(child) != "binary_expression" {
                continue;
            }
            let inner = syntax::operator_str(child);
            if !unclear(outer, inner) {
                continue;
            }
            ctx.emit(
                child,
                format!(
                    "Add parentheses around this `{inner}` operation to show how it groups with `{outer}`"
                ),
            );
        }
    }
}
