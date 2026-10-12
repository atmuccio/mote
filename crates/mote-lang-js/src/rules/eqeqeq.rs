//! `js/eqeqeq`

use crate::prelude::*;
use oxc_syntax::operator::BinaryOperator;

pub struct Eqeqeq;

pub static EQEQEQ: RuleMeta = RuleMeta {
    severity: Severity::Medium,
    tags: &["suspicious"],
    remediation_minutes: 2,
    params: &[ParamSpec {
        name: "allow_null",
        default: ParamDefault::Bool(true),
        description: "Allow `== null` / `!= null`, the idiom for \"null or undefined\".",
    }],
    description: r#"Flags loose equality operators `==` and `!=`.

**Why:** loose equality converts its operands before comparing, with rules few people remember:
`'' == 0`, `'0' == false`, and `[1] == 1` are all `true`. Strict equality (`===`, `!==`) compares
without conversion, so the result is what the code says.

**Fix:** use `===` / `!==`, converting explicitly (`Number(x)`, `String(x)`) where a conversion is
really wanted.

```js
if (count == '0') {}      // flagged
if (count === 0) {}       // compliant
if (value == null) {}     // compliant: matches null and undefined (parameter `allow_null`)
if (typeof x == 'string') {} // compliant: typeof always yields a string
```

Comparisons where conversion cannot happen are not flagged: `typeof` expressions and two literals of
the same type.
"#,
    ..RuleMeta::new("js/eqeqeq", "js", "Use strict equality operators")
};

impl JsRule for Eqeqeq {
    fn meta(&self) -> &'static RuleMeta {
        &EQEQEQ
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::BinaryExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::BinaryExpression(expr) = node.kind() else {
            return;
        };
        if !helpers::is_loose_equality(expr.operator) {
            return;
        }
        let (left, right) = (
            helpers::unwrap_expr(&expr.left),
            helpers::unwrap_expr(&expr.right),
        );
        if helpers::is_typeof(left) || helpers::is_typeof(right) || same_type_literals(left, right)
        {
            return;
        }
        if (left.is_null() || right.is_null()) && ctx.param_bool("allow_null") {
            return;
        }
        let (loose, strict) = if expr.operator == BinaryOperator::Equality {
            ("==", "===")
        } else {
            ("!=", "!==")
        };
        let span = operator_span(ctx, expr.left.span().end, expr.right.span().start, loose)
            .unwrap_or(expr.span);
        ctx.emit(span, format!("Use '{strict}' instead of '{loose}'"));
    }
}

fn same_type_literals(a: &Expression<'_>, b: &Expression<'_>) -> bool {
    let class = |e: &Expression<'_>| match e {
        Expression::StringLiteral(_) => Some(0),
        Expression::TemplateLiteral(t) if t.expressions.is_empty() => Some(0),
        Expression::NumericLiteral(_) => Some(1),
        Expression::BooleanLiteral(_) => Some(2),
        Expression::NullLiteral(_) => Some(3),
        Expression::BigIntLiteral(_) => Some(4),
        _ => None,
    };
    matches!((class(a), class(b)), (Some(x), Some(y)) if x == y)
}

/// Locate the operator token between the operands, skipping comments.
fn operator_span(ctx: &Ctx<'_>, from: u32, to: u32, op: &str) -> Option<Span> {
    let gap = ctx.source().get(from as usize..to as usize)?;
    gap.match_indices(op)
        .map(|(i, _)| from + i as u32)
        .find_map(|start| {
            (!ctx.semantic().is_inside_comment(start))
                .then(|| Span::new(start, start + op.len() as u32))
        })
}
