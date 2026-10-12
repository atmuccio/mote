//! `js/no-useless-catch`

use crate::prelude::*;

pub struct NoUselessCatch;

pub static NO_USELESS_CATCH: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Incubating,
    tags: &["exceptions", "redundant"],
    remediation_minutes: 2,
    description: r#"Flags a `catch` clause whose only statement throws the caught value again, unchanged:
`catch (e) { throw e; }`.

**Why:** the error would have propagated to the caller anyway. The handler does nothing, yet it
looks like error handling, so readers go looking for a purpose it does not have, and the extra
nesting hides the code that matters.

**Fix:** remove the `catch` clause; drop the whole `try` when it has no `finally` either. If the
error does need handling, add it: context in a wrapping error (`new Error('…', { cause: e })`),
cleanup, a fallback value.

```js
try {                          // flagged: the catch only rethrows
  await store.save(order);
} catch (err) {
  throw err;
}

await store.save(order);       // compliant

try {                          // compliant: the error gets context
  await store.save(order);
} catch (err) {
  throw new Error(`saving order ${order.id} failed`, { cause: err });
}
```

Not reported: handlers that do anything else before rethrowing (logging, cleanup), that throw a
different value or a wrapper, and destructured catch parameters.
"#,
    ..RuleMeta::new(
        "js/no-useless-catch",
        "js",
        "A catch clause should do more than rethrow the caught error",
    )
};

impl JsRule for NoUselessCatch {
    fn meta(&self) -> &'static RuleMeta {
        &NO_USELESS_CATCH
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CatchClause]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::CatchClause(clause) = node.kind() else {
            return;
        };
        let [Statement::ThrowStatement(throw)] = clause.body.body.as_slice() else {
            return;
        };
        let Some(param) = &clause.param else {
            return;
        };
        let BindingPattern::BindingIdentifier(binding) = &param.pattern else {
            return;
        };
        let Expression::Identifier(thrown) = helpers::unwrap_expr(&throw.argument) else {
            return;
        };
        if thrown.name != binding.name {
            return;
        }
        let (Some(caught), Some(target)) =
            (ctx.binding_symbol(binding), ctx.reference_symbol(thrown))
        else {
            return;
        };
        if caught != target {
            return;
        }
        let keyword = Span::new(clause.span.start, clause.span.start + 5);
        if ctx.text(keyword) != "catch" {
            return;
        }
        let has_finally = matches!(ctx.parent_kind(node),
            Some(AstKind::TryStatement(t)) if t.finalizer.is_some());
        let message = if has_finally {
            format!(
                "Remove this catch clause: it only rethrows '{}'",
                binding.name
            )
        } else {
            format!(
                "Remove this try/catch: the catch clause only rethrows '{}'",
                binding.name
            )
        };
        ctx.emit(keyword, message);
    }
}
