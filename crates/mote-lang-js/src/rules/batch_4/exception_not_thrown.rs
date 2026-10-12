//! `js/exception-not-thrown`

use crate::prelude::*;

pub struct ExceptionNotThrown;

pub static EXCEPTION_NOT_THROWN: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["error-handling", "suspicious"],
    remediation_minutes: 2,
    description: r#"Flags error objects that are created and immediately discarded: a statement consisting only of
`new Error(…)` or `new SomethingError(…)`.

**Why:** creating an error does nothing by itself. A statement like `new ValidationError('bad
input');` was almost certainly meant to be `throw new ValidationError('bad input');` — as written,
the check it guards silently lets invalid data through.

**Fix:** add the missing `throw` (or `return` the error if the caller is meant to handle it).

```js
if (!user) {
  new Error('user is required');        // flagged: created, never thrown
}

if (!user) {
  throw new Error('user is required');  // compliant
}
const err = new Error('later');         // compliant: the error is kept
```
"#,
    ..RuleMeta::new(
        "js/exception-not-thrown",
        "js",
        "Errors that are created should be thrown",
    )
};

impl JsRule for ExceptionNotThrown {
    fn meta(&self) -> &'static RuleMeta {
        &EXCEPTION_NOT_THROWN
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::ExpressionStatement]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::ExpressionStatement(stmt) = node.kind() else {
            return;
        };
        let Expression::NewExpression(new) = helpers::unwrap_expr(&stmt.expression) else {
            return;
        };
        let name = match helpers::unwrap_expr(&new.callee) {
            Expression::Identifier(id) => id.name.as_str(),
            Expression::StaticMemberExpression(m) => m.property.name.as_str(),
            _ => return,
        };
        if !name.ends_with("Error") {
            return;
        }
        // An arrow function's expression body is also an expression statement: `() => new Error()`
        // returns the error.
        if let Some(parent) = ctx.parent(node) {
            if matches!(parent.kind(), AstKind::FunctionBody(_))
                && matches!(ctx.parent_kind(parent), Some(AstKind::ArrowFunctionExpression(a)) if a.expression)
            {
                return;
            }
        }
        let span = Span::new(new.span.start, new.callee.span().end);
        ctx.emit(
            span,
            format!("Throw this '{name}' or remove it: it is created and immediately discarded"),
        );
    }
}
