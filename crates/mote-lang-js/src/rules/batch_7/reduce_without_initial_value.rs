//! `js/reduce-without-initial-value`

use super::util::is_array_factory;
use crate::prelude::*;

pub struct ReduceWithoutInitialValue;

pub static REDUCE_WITHOUT_INITIAL_VALUE: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["suspicious"],
    remediation_minutes: 2,
    description: r#"Flags `reduce(fn)` and `reduceRight(fn)` without an initial value on an array that can plainly be
empty: the result of `filter(…)`, of `Object.keys/values/entries(…)`, or an empty literal.

**Why:** without an initial value, `reduce` starts from the first element — and throws
`TypeError: Reduce of empty array with no initial value` when there is none. A filter that
matches nothing, or an object without keys, is a normal situation, so the code works in tests and
fails on the first empty input in production.

**Fix:** pass the initial value as the second argument (`0` for a sum, `''` for a join, `{}` for
an object being built), or check for an empty array first.

```js
const total = items.filter(isPaid).reduce((sum, x) => sum + x.amount);     // flagged
const total = items.filter(isPaid).reduce((sum, x) => sum + x.amount, 0);  // compliant
```

Receivers whose emptiness cannot be seen from the call (variables, parameters, other calls) are
not reported.
"#,
    ..RuleMeta::new(
        "js/reduce-without-initial-value",
        "js",
        "'reduce' on a possibly empty array should have an initial value",
    )
};

impl JsRule for ReduceWithoutInitialValue {
    fn meta(&self) -> &'static RuleMeta {
        &REDUCE_WITHOUT_INITIAL_VALUE
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CallExpression]
    }

    fn callee_names(&self) -> &'static [&'static str] {
        &["reduce", "reduceRight"]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::CallExpression(call) = node.kind() else {
            return;
        };
        if call.arguments.len() != 1 || matches!(call.arguments[0], Argument::SpreadElement(_)) {
            return;
        }
        let Expression::StaticMemberExpression(member) = helpers::unwrap_expr(&call.callee) else {
            return;
        };
        let method = member.property.name.as_str();
        if method != "reduce" && method != "reduceRight" {
            return;
        }
        let source = match helpers::unwrap_expr(&member.object) {
            Expression::ArrayExpression(a) if a.elements.is_empty() => "an empty array",
            Expression::CallExpression(inner) => match helpers::unwrap_expr(&inner.callee) {
                Expression::StaticMemberExpression(m)
                    if m.property.name == "filter" && !inner.arguments.is_empty() =>
                {
                    "the result of 'filter()', which can be empty"
                }
                _ if is_array_factory(ctx, inner)
                    && matches!(
                        helpers::call_member_names(inner),
                        Some((_, "keys" | "values" | "entries"))
                    ) =>
                {
                    "the entries of an object, which can have none"
                }
                _ => return,
            },
            _ => return,
        };
        ctx.emit(
            member.property.span,
            format!(
                "Pass an initial value to '{method}()': it is called on {source}, and throws a TypeError when there is nothing to reduce"
            ),
        );
    }
}
