//! `js/require-yield`

use super::util;
use crate::prelude::*;

pub struct RequireYield;

pub static REQUIRE_YIELD: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["generators", "suspicious"],
    remediation_minutes: 5,
    description: r#"Flags generator functions (`function*`) whose body never uses `yield`.

**Why:** calling a generator does not run its body; it returns an iterator, and the body only runs
as the iterator is consumed. A generator without `yield` produces no values: loops over it do
nothing, and its code runs lazily — or never, if nobody iterates. Usually the `*` is left over from
a refactoring or the `yield` was forgotten (for example `return item` written instead of
`yield item` inside a loop).

**Fix:** add the missing `yield`, or remove the `*` and make it a plain function.

```js
function* ids(items) {       // flagged: nothing is ever yielded
  for (const item of items) {
    return item.id;
  }
}

function* ids(items) {       // compliant
  for (const item of items) {
    yield item.id;
  }
}
```

Not reported: generators with an empty body or a body that is a single `return` or `throw` (a
placeholder or an empty sequence), and generators that are class or object methods or property
values: those usually implement a protocol — an interface, a base class, `[Symbol.iterator]` —
where yielding nothing is a legitimate answer.
"#,
    ..RuleMeta::new("js/require-yield", "js", "Generator functions should yield")
};

impl JsRule for RequireYield {
    fn meta(&self) -> &'static RuleMeta {
        &REQUIRE_YIELD
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::Function]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::Function(function) = node.kind() else {
            return;
        };
        if !function.generator {
            return;
        }
        let Some(body) = &function.body else {
            return;
        };
        match body.statements.as_slice() {
            [] | [Statement::ThrowStatement(_) | Statement::ReturnStatement(_)] => return,
            _ => {}
        }
        // Methods and property values usually implement a protocol (an interface, a base class, an
        // `[Symbol.iterator]`) where producing no values is a legitimate answer.
        if matches!(
            ctx.parent_kind(node),
            Some(
                AstKind::MethodDefinition(_)
                    | AstKind::ObjectProperty(_)
                    | AstKind::PropertyDefinition(_)
            )
        ) {
            return;
        }
        let yields = util::descendants(ctx, node).any(|inner| {
            matches!(inner.kind(), AstKind::YieldExpression(_))
                && util::this_owner(ctx, inner).map(AstNode::id) == Some(node.id())
        });
        if yields {
            return;
        }
        let span = match &function.id {
            Some(id) => id.span,
            None => Span::new(function.span.start, function.params.span.start),
        };
        ctx.emit(
            span,
            "Add a 'yield' to this generator or remove the '*': it never produces a value",
        );
    }
}
