//! `js/prefer-spread`

use crate::prelude::*;
use oxc_span::ContentEq;

pub struct PreferSpread;

pub static PREFER_SPREAD: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["es2015", "convention"],
    remediation_minutes: 2,
    description: r#"Flags `Function.prototype.apply` calls that only exist to pass an array as the
argument list: `fn.apply(null, args)`, `obj.method.apply(obj, args)`.

**Why:** since ES2015 the spread operator does the same thing directly, `fn(...args)`, without
repeating the receiver and without the `null`/`undefined` placeholder that readers must decode.

**Fix:** call the function with spread arguments.

```js
Math.max.apply(Math, values);          // flagged
logger.warn.apply(logger, parts);      // flagged
fn.apply(undefined, args);             // flagged
handlers[name].apply(handlers, args);  // flagged
makeHandler().apply(null, args);       // flagged

Math.max(...values);                   // compliant
logger.warn(...parts);                 // compliant
handler.apply(context, args);          // compliant: a different `this` is the point
```

Only calls whose `this` argument is exactly what a plain call would use are reported: the object
the method is read from (by name or by a computed key), or `null`/`undefined` for a plain function
or a function returned by a call.
"#,
    ..RuleMeta::new(
        "js/prefer-spread",
        "js",
        "Spread syntax should be used instead of '.apply()'",
    )
};

impl JsRule for PreferSpread {
    fn meta(&self) -> &'static RuleMeta {
        &PREFER_SPREAD
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CallExpression]
    }

    fn callee_names(&self) -> &'static [&'static str] {
        &["apply"]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::CallExpression(call) = node.kind() else {
            return;
        };
        let Expression::StaticMemberExpression(member) = &call.callee else {
            return;
        };
        if member.property.name != "apply" || member.optional || call.optional {
            return;
        }
        let [this_arg, list] = call.arguments.as_slice() else {
            return;
        };
        let (Some(this_arg), Some(list)) = (this_arg.as_expression(), list.as_expression()) else {
            return;
        };
        // `fn.apply(x, [a, b])` is better written as a direct call, but that is not a spread.
        if matches!(helpers::unwrap_expr(list), Expression::ArrayExpression(_)) {
            return;
        }
        let applied = helpers::unwrap_expr(&member.object);
        let this_arg = helpers::unwrap_expr(this_arg);
        let matches_plain_call = match applied {
            Expression::StaticMemberExpression(m) => {
                is_simple_receiver(&m.object)
                    && helpers::unwrap_expr(&m.object).content_eq(this_arg)
            }
            // `obj[key].apply(obj, args)` is `obj[key](...args)`.
            Expression::ComputedMemberExpression(m) => {
                is_simple_receiver(&m.object)
                    && helpers::unwrap_expr(&m.object).content_eq(this_arg)
            }
            Expression::PrivateFieldExpression(_) | Expression::ChainExpression(_) => false,
            // `make().apply(null, args)` is `make()(...args)`.
            Expression::Identifier(_) | Expression::CallExpression(_) => {
                this_arg.is_null() || this_arg.is_undefined() || this_arg.is_void_0()
            }
            _ => false,
        };
        if !matches_plain_call {
            return;
        }
        ctx.emit(
            member.property.span,
            format!(
                "Call '{}' with spread arguments instead of '.apply()'",
                super::util::preview(ctx.text(applied.span()), 40)
            ),
        );
    }
}

/// A receiver that evaluates the same way twice: `this`, a name, or a chain of plain property
/// reads on those.
fn is_simple_receiver(expr: &Expression<'_>) -> bool {
    match helpers::unwrap_expr(expr) {
        Expression::ThisExpression(_) | Expression::Identifier(_) => true,
        Expression::StaticMemberExpression(m) => is_simple_receiver(&m.object),
        _ => false,
    }
}
