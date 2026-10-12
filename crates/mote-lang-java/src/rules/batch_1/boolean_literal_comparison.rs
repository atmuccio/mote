//! `java/boolean-literal-comparison` — `x == true`, `c ? true : false`.

use super::util::{self, Fld, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["clutter", "readability"],
    remediation_minutes: 1,
    description: r#"Detects comparisons with a boolean literal (`x == true`, `x != false`,
`false == x`), conditional expressions that just produce a boolean literal
(`c ? true : false`, `c ? false : true`), boolean literals that a logical operator ignores
(`x && true`, `x || false`), and negated literals (`!false`).

**Why it matters.** The literal adds nothing: the expression is already a boolean. The extra
words make conditions longer and invite mistakes such as `if (x = true)`.

**How to fix.** Use the value directly, negating it where needed.

```java
// Noncompliant
if (isValid() == true) { ... }
if (enabled != false) { ... }
return count > 0 ? true : false;
if (ready && true) { ... }

// Compliant
if (isValid()) { ... }
if (enabled) { ... }
return count > 0;
if (ready) { ... }
```

`x && false` and `x || true` are not reported here: the literal decides the whole condition,
which is a constant condition rather than clutter.

Note that for a `Boolean` object both forms throw on `null`; if `null` must be treated as
`false`, write `Boolean.TRUE.equals(x)`."#,
    ..RuleMeta::new(
        "java/boolean-literal-comparison",
        "java",
        "Comparison with a boolean literal",
    )
};

pub struct BooleanLiteralComparison;

fn bool_literal(n: Node<'_>) -> Option<bool> {
    match kind(syntax::strip_parens(n)) {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

impl<'t> JavaRule<'t> for BooleanLiteralComparison {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "binary_expression",
            "ternary_expression",
            "unary_expression",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if kind(node) == "ternary_expression" {
            let (Some(a), Some(b)) = (node.fld(F::Consequence), node.fld(F::Alternative)) else {
                return;
            };
            match (bool_literal(a), bool_literal(b)) {
                (Some(true), Some(false)) => {
                    ctx.emit(node, "Use the condition itself instead of `? true : false`");
                }
                (Some(false), Some(true)) => {
                    ctx.emit(
                        node,
                        "Use the negated condition instead of `? false : true`",
                    );
                }
                _ => {}
            }
            return;
        }
        let op = ctx.operator_str(node);
        if kind(node) == "unary_expression" {
            if op == "!" {
                if let Some(v) = node.fld(F::Operand).and_then(bool_literal) {
                    ctx.emit(
                        node,
                        format!("Write `{}` instead of negating a literal", !v),
                    );
                }
            }
            return;
        }
        if op == "&&" || op == "||" {
            let (Some(left), Some(right)) = (node.fld(F::Left), node.fld(F::Right)) else {
                return;
            };
            // The literal the operator ignores: `&& true`, `|| false`.
            let neutral = op == "&&";
            if bool_literal(left) == Some(neutral) || bool_literal(right) == Some(neutral) {
                let at = util::op(node).unwrap_or(node);
                ctx.emit(
                    at,
                    format!("Remove `{op} {neutral}`: it does not change the condition"),
                );
            }
            return;
        }
        if op != "==" && op != "!=" {
            return;
        }
        let (Some(left), Some(right)) = (node.fld(F::Left), node.fld(F::Right)) else {
            return;
        };
        let (lit, other) = match (bool_literal(left), bool_literal(right)) {
            (Some(v), None) => (v, right),
            (None, Some(v)) => (v, left),
            _ => return,
        };
        if kind(syntax::strip_parens(other)) == "null_literal" {
            return;
        }
        let keep = (op == "==") == lit;
        let msg = if keep {
            "Remove the comparison with a boolean literal; use the value directly"
        } else {
            "Remove the comparison with a boolean literal; negate the value instead"
        };
        let at = util::op(node).unwrap_or(node);
        ctx.emit(at, msg);
    }
}
