//! `js/no-compound-case-label`

use crate::prelude::*;

pub struct NoCompoundCaseLabel;

pub static NO_COMPOUND_CASE_LABEL: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["switch", "suspicious"],
    remediation_minutes: 5,
    description: r#"Flags a `case` label that tries to list several values with a comma or a logical operator:
`case 'a', 'b':` or `case 'a' || 'b':`.

**Why:** a `case` compares the `switch` value with *one* value, the result of its expression. The
comma operator evaluates to its last operand, so `case 1, 2:` only matches `2`; `'a' || 'b'`
evaluates to `'a'`, so `case 'a' || 'b':` only matches `'a'`. The other values silently fall to
`default`.

**Fix:** write one `case` per value; consecutive labels share the same body.

```js
switch (key) {
  case 'Enter', 'Space':     // flagged: only matches 'Space'
    submit();
    break;
  case 'Up' || 'Down':       // flagged: only matches 'Up'
    move();
    break;
}

switch (key) {
  case 'Enter':
  case 'Space':              // compliant
    submit();
    break;
}
```

A logical operator is only reported when both operands are constants (literals, `UPPER_CASE`
names, or enum-like members such as `Keys.Up`), and never in `switch (true)`, where each `case`
holds a whole condition.
"#,
    ..RuleMeta::new(
        "js/no-compound-case-label",
        "js",
        "Case labels should hold a single value",
    )
};

impl JsRule for NoCompoundCaseLabel {
    fn meta(&self) -> &'static RuleMeta {
        &NO_COMPOUND_CASE_LABEL
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::SwitchCase]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::SwitchCase(case) = node.kind() else {
            return;
        };
        let Some(test) = &case.test else {
            return;
        };
        let Some(AstKind::SwitchStatement(switch)) = ctx.parent_kind(node) else {
            return;
        };
        if matches!(
            helpers::unwrap_expr(&switch.discriminant),
            Expression::BooleanLiteral(_)
        ) {
            return;
        }
        match helpers::unwrap_expr(test) {
            Expression::SequenceExpression(seq) => {
                ctx.emit(
                    seq.span,
                    "Write a separate 'case' for each value: a comma expression only matches its last value",
                );
            }
            Expression::LogicalExpression(l) => {
                if !is_constant(&l.left) || !is_constant(&l.right) {
                    return;
                }
                let op = l.operator.as_str();
                let which = match l.operator {
                    LogicalOperator::And => "its last value",
                    LogicalOperator::Or | LogicalOperator::Coalesce => "its first value",
                };
                ctx.emit(
                    l.span,
                    format!("Write a separate 'case' for each value: '{op}' makes this label match only {which}"),
                );
            }
            _ => {}
        }
    }
}

/// A literal or a constant-looking name (`MAX`, `Keys.Up`, `Direction.UP`).
fn is_constant(expr: &Expression<'_>) -> bool {
    match helpers::unwrap_expr(expr) {
        Expression::StringLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BigIntLiteral(_)
        | Expression::NullLiteral(_) => true,
        Expression::TemplateLiteral(t) => t.expressions.is_empty(),
        Expression::UnaryExpression(u) => {
            u.operator == UnaryOperator::UnaryNegation
                && matches!(u.argument, Expression::NumericLiteral(_))
        }
        Expression::Identifier(id) => is_upper_case(&id.name),
        Expression::StaticMemberExpression(m) => {
            matches!(
                &m.object,
                Expression::Identifier(_) | Expression::StaticMemberExpression(_)
            ) && m
                .property
                .name
                .chars()
                .next()
                .is_some_and(char::is_uppercase)
        }
        Expression::LogicalExpression(l) => is_constant(&l.left) && is_constant(&l.right),
        _ => false,
    }
}

fn is_upper_case(name: &str) -> bool {
    name.len() > 1
        && name.chars().any(|c| c.is_ascii_uppercase())
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}
