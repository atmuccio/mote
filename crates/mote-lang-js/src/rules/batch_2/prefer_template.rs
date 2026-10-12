//! `js/prefer-template`

use super::util;
use crate::prelude::*;

pub struct PreferTemplate;

pub static PREFER_TEMPLATE: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["convention", "conventions"],
    remediation_minutes: 2,
    default_enabled: false,
    description: r#"Flags string concatenation with `+` that mixes string literals with other values.

**Why:** a template literal shows the resulting text in one piece, with the values in place, and
avoids the easy mistakes of concatenation: missing spaces, forgotten `+`, and accidental numeric
addition (`'total: ' + a + b`).

**Fix:** write a template literal: `` `Hello ${name}!` ``. Each concatenation chain is reported
once. Concatenations of literals only are left to `js/no-useless-concat`.

```js
const msg = 'Hello ' + name + '!';     // flagged
const msg = `Hello ${name}!`;          // compliant
const sum = a + b;                     // compliant: no string literal
```
"#,
    ..RuleMeta::new(
        "js/prefer-template",
        "js",
        "Template literals should be used instead of string concatenation",
    )
};

fn addition<'b, 'a>(expr: &'b Expression<'a>) -> Option<&'b BinaryExpression<'a>> {
    match helpers::unwrap_expr(expr) {
        Expression::BinaryExpression(b) if b.operator == BinaryOperator::Addition => Some(b),
        _ => None,
    }
}

/// What kinds of operands a `+` chain has.
#[derive(Default)]
struct Operands {
    literal: bool,
    other: bool,
}

impl Operands {
    fn classify(&mut self, operand: &Expression<'_>) {
        if util::is_string_literal(operand) {
            self.literal = true;
        } else {
            self.other = true;
        }
    }

    /// Walk the chain: iteratively down the left spine (`a + b + c` nests to the left), and
    /// recursively into the rare parenthesized right operand (`a + (b + c)`), with a depth cap.
    fn scan(&mut self, chain: &BinaryExpression<'_>, depth: u32) {
        let mut current = chain;
        loop {
            match addition(&current.right) {
                Some(right) if depth < 32 => self.scan(right, depth + 1),
                Some(_) => self.other = true,
                None => self.classify(&current.right),
            }
            match addition(&current.left) {
                Some(left) => current = left,
                None => {
                    self.classify(&current.left);
                    return;
                }
            }
        }
    }
}

impl JsRule for PreferTemplate {
    fn meta(&self) -> &'static RuleMeta {
        &PREFER_TEMPLATE
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::BinaryExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::BinaryExpression(expr) = node.kind() else {
            return;
        };
        if expr.operator != BinaryOperator::Addition {
            return;
        }
        // Only the outermost `+` of a chain.
        if let Some(AstKind::BinaryExpression(parent)) = ctx.outer_parent(node).map(AstNode::kind) {
            if parent.operator == BinaryOperator::Addition {
                return;
            }
        }
        let mut kinds = Operands::default();
        kinds.scan(expr, 0);
        if kinds.literal && kinds.other {
            ctx.emit(
                expr.span,
                "Use a template literal instead of string concatenation",
            );
        }
    }
}
