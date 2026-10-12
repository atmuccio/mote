//! `js/misleading-array-mutation`

use super::util::is_visible_array;
use crate::prelude::*;

pub struct MisleadingArrayMutation;

pub static MISLEADING_ARRAY_MUTATION: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["suspicious"],
    remediation_minutes: 5,
    description: r#"Flags `const sorted = list.sort()` (likewise `reverse`, `fill` and `copyWithin`) when `list` is used
again afterwards.

**Why:** these methods change the array in place and return that same array, not a copy.
Storing the result under a new name reads as "a sorted copy, the original stays as it was", but
both names now refer to one reordered array — every later use of the original sees the new
order, which is rarely what the code that follows expects.

**Fix:** sort a copy — `list.toSorted()`, `list.toReversed()`, `[...list].sort()` — or, if the
change in place is intended, drop the second name and keep using the original.

```js
const names = ['b', 'a'];
const sorted = names.sort();          // flagged: `names` is sorted too
render(names, sorted);

const sorted = [...names].sort();     // compliant
const sorted = names.toSorted();      // compliant
```

Only arrays the code visibly creates or declares are checked (array literals, `Array.from`,
`Object.keys`, variables or parameters with an array type), and only when the original array is
read again after the call in a way that depends on its order — `length`, `includes`, `some`,
`every`, or spreading it into an array that is sorted again do not count. `splice` is not reported:
its result is the removed elements, which is exactly what it is used for.
"#,
    ..RuleMeta::new(
        "js/misleading-array-mutation",
        "js",
        "Results of in-place array methods should not be stored as if they were copies",
    )
};

impl JsRule for MisleadingArrayMutation {
    fn meta(&self) -> &'static RuleMeta {
        &MISLEADING_ARRAY_MUTATION
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CallExpression]
    }

    fn callee_names(&self) -> &'static [&'static str] {
        &["sort", "reverse", "fill", "copyWithin"]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::CallExpression(call) = node.kind() else {
            return;
        };
        let Expression::StaticMemberExpression(member) = helpers::unwrap_expr(&call.callee) else {
            return;
        };
        let method = member.property.name.as_str();
        if !matches!(method, "sort" | "reverse" | "fill" | "copyWithin") {
            return;
        }
        let Expression::Identifier(receiver) = helpers::unwrap_expr(&member.object) else {
            return;
        };
        // Where does the result go? A new variable, or another assignment target.
        let Some(target_name) = stored_under(ctx, node, receiver.name.as_str()) else {
            return;
        };
        let Some(symbol) = ctx.reference_symbol(receiver) else {
            return;
        };
        if !is_visible_array(ctx, &member.object) {
            return;
        }
        let end = call.span.end;
        let Some(later) = ctx.references_of(symbol).find(|r| {
            if !r.is_read() {
                return false;
            }
            let reference = ctx.node(r.node_id());
            reference.span().start >= end && !is_order_insensitive(ctx, reference)
        }) else {
            return;
        };
        let later_span = ctx.node(later.node_id()).span();
        let name = receiver.name.as_str();
        ctx.emit(
            member.property.span,
            format!(
                "'{method}()' changes '{name}' in place and returns it: '{target_name}' is not a copy, and '{name}' is used again below; call it on a copy (or use the 'to…' variant) if the original must stay unchanged"
            ),
        )
        .secondary(later_span, format!("'{name}' is used again here"));
    }
}

/// A use that does not depend on the order of the elements: `list.length`, `list.includes(…)`,
/// `list.some(…)`, `list.every(…)`, sorting it again, or spreading it into an array that is then
/// sorted (`[...list, ...more].sort()`).
fn is_order_insensitive(ctx: &Ctx<'_>, reference: &AstNode<'_>) -> bool {
    let Some(parent) = ctx.parent(reference) else {
        return false;
    };
    match parent.kind() {
        AstKind::StaticMemberExpression(m) => matches!(
            m.property.name.as_str(),
            "length" | "includes" | "some" | "every" | "sort" | "toSorted"
        ),
        AstKind::SpreadElement(_) => {
            let Some(array) = ctx.parent(parent) else {
                return false;
            };
            matches!(array.kind(), AstKind::ArrayExpression(_))
                && matches!(
                    ctx.outer_parent(array).map(AstNode::kind),
                    Some(AstKind::StaticMemberExpression(m))
                        if matches!(m.property.name.as_str(), "sort" | "toSorted")
                )
        }
        _ => false,
    }
}

/// The name the call's result is stored under: `const x = a.sort()`, `x = a.sort()`,
/// `this.x = a.sort()`. `None` for other uses, or when it is stored back into the receiver.
fn stored_under<'a>(ctx: &Ctx<'a>, call: &AstNode<'a>, receiver: &str) -> Option<&'a str> {
    let parent = ctx.outer_parent(call)?;
    match parent.kind() {
        AstKind::VariableDeclarator(d) => match &d.id {
            BindingPattern::BindingIdentifier(id) if id.name != receiver => Some(id.name.as_str()),
            _ => None,
        },
        AstKind::AssignmentExpression(a) if a.operator == AssignmentOperator::Assign => {
            match &a.left {
                AssignmentTarget::AssignmentTargetIdentifier(id) if id.name != receiver => {
                    Some(id.name.as_str())
                }
                AssignmentTarget::StaticMemberExpression(m) => Some(m.property.name.as_str()),
                _ => None,
            }
        }
        _ => None,
    }
}
