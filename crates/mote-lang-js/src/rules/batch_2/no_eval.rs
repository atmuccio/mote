//! `js/no-eval`

use crate::prelude::*;

pub struct NoEval;

pub static NO_EVAL: RuleMeta = RuleMeta {
    severity: Severity::Medium,
    tags: &["security"],
    remediation_minutes: 15,
    description: r#"Flags code that compiles strings at run time: `eval(…)`, `Function(…)` /
`new Function(…)`, and `setTimeout` / `setInterval` called with a string instead of a function
(also through `window.`, `self.`, or `globalThis.`).

**Why:** evaluating strings as code opens the door to code injection when any part of the string
comes from outside, defeats static analysis, minification, and Content Security Policies, and is
slow.

**Fix:** use a function (`setTimeout(() => tick(), 100)`), a lookup object instead of building
names dynamically, or `JSON.parse` for data. Local functions named `eval` or `Function` are not
reported; `setTimeout` is only reported when its first argument is visibly a string.

```js
eval('run' + step + '()');              // flagged
const add = new Function('a', 'b', 'return a + b');   // flagged
setTimeout('refresh()', 1000);          // flagged

steps[step]();                          // compliant
setTimeout(refresh, 1000);              // compliant
```
"#,
    ..RuleMeta::new(
        "js/no-eval",
        "js",
        "Strings should not be evaluated as code",
    )
};

/// Names of the globals that evaluate strings as code.
fn is_evaluating(name: &str) -> bool {
    matches!(
        name,
        "eval" | "Function" | "setTimeout" | "setInterval" | "execScript"
    )
}

/// `window.x`, `self.x`, `globalThis.x`, or a plain global `x` for such a name.
fn global_callee<'a>(ctx: &Ctx<'a>, callee: &Expression<'a>) -> Option<&'a str> {
    match helpers::unwrap_expr(callee) {
        Expression::Identifier(id) => {
            let name = id.name.as_str();
            (is_evaluating(name) && ctx.is_global_reference(id)).then_some(name)
        }
        Expression::StaticMemberExpression(m) => {
            let name = m.property.name.as_str();
            let on_global = is_evaluating(name)
                && (ctx.is_global(&m.object, "window")
                    || ctx.is_global(&m.object, "self")
                    || ctx.is_global(&m.object, "globalThis"));
            on_global.then_some(name)
        }
        _ => None,
    }
}

/// A string literal, a template literal, or a `+` chain with one as an operand.
fn is_string_like(expr: &Expression<'_>) -> bool {
    let mut current = expr;
    loop {
        match helpers::unwrap_expr(current) {
            Expression::StringLiteral(_) | Expression::TemplateLiteral(_) => return true,
            Expression::BinaryExpression(b) if b.operator == BinaryOperator::Addition => {
                if matches!(
                    helpers::unwrap_expr(&b.right),
                    Expression::StringLiteral(_) | Expression::TemplateLiteral(_)
                ) {
                    return true;
                }
                current = &b.left;
            }
            _ => return false,
        }
    }
}

impl JsRule for NoEval {
    fn meta(&self) -> &'static RuleMeta {
        &NO_EVAL
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CallExpression, AstType::NewExpression]
    }

    fn callee_names(&self) -> &'static [&'static str] {
        &[
            "eval",
            "Function",
            "setTimeout",
            "setInterval",
            "execScript",
        ]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let (callee, arguments, is_new) = match node.kind() {
            AstKind::CallExpression(c) => (&c.callee, &c.arguments, false),
            AstKind::NewExpression(n) => (&n.callee, &n.arguments, true),
            _ => return,
        };
        let Some(name) = global_callee(ctx, callee) else {
            return;
        };
        let message = match name {
            "eval" if !is_new => "Remove this use of 'eval': it runs a string as code",
            "Function" => {
                "Remove this use of the 'Function' constructor: it compiles a string as code"
            }
            "setTimeout" | "setInterval" | "execScript" if !is_new => {
                let first = arguments.first().and_then(Argument::as_expression);
                if !first.is_some_and(is_string_like) {
                    return;
                }
                "Pass a function instead of a string: this string is evaluated as code"
            }
            _ => return,
        };
        ctx.emit(callee.span(), message);
    }
}
