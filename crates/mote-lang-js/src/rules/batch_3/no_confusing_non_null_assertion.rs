//! `js/no-confusing-non-null-assertion`

use crate::prelude::*;

pub struct NoConfusingNonNullAssertion;

pub static NO_CONFUSING_NON_NULL_ASSERTION: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["typescript", "confusing"],
    remediation_minutes: 2,
    description: r#"Flags a non-null assertion written directly before `==`, `===`, or `=`
(`a! == b`, `a?.b! === c`, `a! = b`).

**Why:** the `!` next to the operator looks like part of it, so `a! == b` reads as `a != b` — the
exact opposite of what the code does. The assertion itself also has no effect on a comparison, and
readers of `a! = b` easily miss that this is an assignment.

**Fix:** if you meant "not equal", write `!=` / `!==`. Otherwise remove the `!`, or wrap the
operand in parentheses to make the assertion explicit.

```ts
if (user.id! == otherId) {}     // flagged: looks like `!=`
config.value! = computeValue(); // flagged

if (user.id == otherId) {}      // compliant
if (user.id != otherId) {}      // compliant: the intended "not equal"
(config.value!) = computeValue(); // compliant: the assertion is explicit
```
"#,
    ..RuleMeta::new(
        "js/no-confusing-non-null-assertion",
        "js",
        "Non-null assertions should not be placed next to an equals sign",
    )
};

impl JsRule for NoConfusingNonNullAssertion {
    fn meta(&self) -> &'static RuleMeta {
        &NO_CONFUSING_NON_NULL_ASSERTION
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::BinaryExpression, AstType::AssignmentExpression]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.is_typescript()
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let (left, operator) = match node.kind() {
            AstKind::BinaryExpression(bin) => {
                let op = match bin.operator {
                    BinaryOperator::Equality => "==",
                    BinaryOperator::StrictEquality => "===",
                    _ => return,
                };
                match &bin.left {
                    Expression::TSNonNullExpression(e) => (e.span, op),
                    // `a?.b! == c`: the assertion is the last link of the optional chain.
                    Expression::ChainExpression(chain) => match &chain.expression {
                        ChainElement::TSNonNullExpression(e) => (e.span, op),
                        _ => return,
                    },
                    _ => return,
                }
            }
            AstKind::AssignmentExpression(assign) => {
                if assign.operator != AssignmentOperator::Assign {
                    return;
                }
                match &assign.left {
                    AssignmentTarget::TSNonNullExpression(e) => (e.span, "="),
                    _ => return,
                }
            }
            _ => return,
        };
        // `(a!) == b` has the same tree; only the unparenthesized form is confusing.
        if super::util::next_char(ctx, left.end) != Some('=') {
            return;
        }
        let Some(bang) = super::util::bang_span(ctx, left) else {
            return;
        };
        let looks_like = if operator == "===" { "!==" } else { "!=" };
        ctx.emit(
            bang,
            format!(
                "Remove this '!' or parenthesize the operand: 'a! {operator} b' is easily read as 'a {looks_like} b'"
            ),
        );
    }
}
