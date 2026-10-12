//! `js/reversed-compound-operator`

use crate::prelude::*;

pub struct ReversedCompoundOperator;

pub static REVERSED_COMPOUND_OPERATOR: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["suspicious", "typo"],
    remediation_minutes: 2,
    description: r#"Flags assignments written as `x =+ y`, `x =- y`, or `x =! y`: the characters of a compound
operator (`+=`, `-=`, `!=`) typed in the wrong order.

**Why:** `x =+ y` is valid code, but it means `x = +y` — the value of `y` replaces `x` instead of
being added to it. Likewise `x =- 1` sets `x` to `-1`, and `x =! y` stores the negation of `y`
where a comparison was probably meant. The code runs without error and computes the wrong value.

**Fix:** swap the characters (`x += y`), or, if the unary operator is intended, write it next to
its operand with a space after the `=` (`x = -1`).

```js
total =+ item.price;    // flagged: sets total to the price
count =- 1;             // flagged: sets count to -1

total += item.price;    // compliant
count = -1;             // compliant: spacing shows the intent
```

Only the typo layout is reported: the unary operator directly after `=` and followed by a space.
"#,
    ..RuleMeta::new(
        "js/reversed-compound-operator",
        "js",
        "Compound assignment operators should not be reversed",
    )
};

impl JsRule for ReversedCompoundOperator {
    fn meta(&self) -> &'static RuleMeta {
        &REVERSED_COMPOUND_OPERATOR
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.file_contains(AstType::UnaryExpression)
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::AssignmentExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::AssignmentExpression(assign) = node.kind() else {
            return;
        };
        if assign.operator != AssignmentOperator::Assign {
            return;
        }
        let Expression::UnaryExpression(unary) = &assign.right else {
            return;
        };
        let op = match unary.operator {
            UnaryOperator::UnaryPlus => '+',
            UnaryOperator::UnaryNegation => '-',
            UnaryOperator::LogicalNot => '!',
            _ => return,
        };
        // `=` immediately followed by the operator, then whitespace: `x =+ y`.
        let start = unary.span.start;
        if start == 0 {
            return;
        }
        let around = ctx.text(Span::new(start - 1, start + 2));
        let mut chars = around.chars();
        if chars.next() != Some('=')
            || chars.next() != Some(op)
            || !chars.next().is_some_and(char::is_whitespace)
        {
            return;
        }
        let compound = if op == '!' {
            "!="
        } else if op == '+' {
            "+="
        } else {
            "-="
        };
        ctx.emit(
            Span::new(start - 1, start + 1),
            format!(
                "Did you mean '{compound}'? '={op}' assigns '{op}' applied to the right-hand side"
            ),
        );
    }
}
