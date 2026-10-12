//! `js/no-console` — scope-aware (`console` must be the global) and relaxed for test files.

use crate::prelude::*;

pub struct NoConsole;

pub static NO_CONSOLE: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["logging"],
    remediation_minutes: 2,
    params: &[ParamSpec {
        name: "allow",
        default: ParamDefault::Str(""),
        description: "Comma-separated console methods that are allowed, e.g. `warn,error`.",
    }],
    description: r#"Flags calls to `console` methods (`console.log`, `console.debug`, …) outside test files.

**Why:** console output left in application code is usually leftover debugging. It clutters the
output users and operators see, may leak data, and cannot be filtered or routed like proper logging.

**Fix:** remove the call, or use the project's logger. Methods listed in the `allow` parameter (for
example `warn,error`) are accepted. A local variable or parameter named `console` is not the global
console and is not flagged. Calls inside a class that is itself a console-backed logger are not
flagged either: a class whose name ends in `Logger`, `LoggerService`, `Log` or `Console`
(`FormattedConsoleLogger`, `DebugConsole`), or that extends or implements a type named that way
(`extends AbstractLogger`, `implements ILogger`), since writing to the console is its purpose.
Other classes that merely log (`LoggingInterceptor`, `LoggerMiddleware`) are still reported.

```js
console.log('user', user);   // flagged
logger.debug('user', user);  // compliant
```
"#,
    ..RuleMeta::new("js/no-console", "js", "Console calls should be removed")
};

impl JsRule for NoConsole {
    fn meta(&self) -> &'static RuleMeta {
        &NO_CONSOLE
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CallExpression]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        !ctx.is_test()
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::CallExpression(call) = node.kind() else {
            return;
        };
        let callee = helpers::unwrap_expr(&call.callee);
        let (object, method) = match callee {
            Expression::StaticMemberExpression(m) => (&m.object, m.property.name.as_str()),
            Expression::ComputedMemberExpression(m) => match helpers::string_value(&m.expression) {
                Some(name) => (&m.object, name),
                None => return,
            },
            _ => return,
        };
        if !ctx.is_global(object, "console") || ctx.param_list("allow").any(|m| m == method) {
            return;
        }
        if in_logger_class(ctx, node) {
            return;
        }
        ctx.emit(
            callee.span(),
            format!("Remove this 'console.{method}' call or use a logger"),
        );
    }
}

/// Is the call inside a class that is itself a console-backed logger: named like one
/// (`ConsoleLogger`, `DebugLog`, `VirtualConsole`), or extending or implementing one
/// (`extends AbstractLogger`, `implements ILogger`)? Writing to the console is its job.
fn in_logger_class(ctx: &Ctx<'_>, node: &AstNode<'_>) -> bool {
    let Some(class) = ctx.ancestors(node).find_map(|n| match n.kind() {
        AstKind::Class(c) => Some(c),
        _ => None,
    }) else {
        return false;
    };
    class.id.as_ref().is_some_and(|id| is_logger_name(&id.name))
        || class
            .super_class
            .as_ref()
            .is_some_and(|s| match helpers::unwrap_expr(s) {
                Expression::Identifier(id) => is_logger_name(&id.name),
                Expression::StaticMemberExpression(m) => is_logger_name(&m.property.name),
                _ => false,
            })
        || class.implements.iter().any(|i| match &i.expression {
            TSTypeName::IdentifierReference(id) => is_logger_name(&id.name),
            TSTypeName::QualifiedName(q) => is_logger_name(&q.right.name),
            TSTypeName::ThisExpression(_) => false,
        })
}

/// `Logger`, `ConsoleLogger`, `ILogger`, `LoggerService`, `ConsoleLog`, `DebugConsole`.
fn is_logger_name(name: &str) -> bool {
    ["Logger", "LoggerService", "Log", "Console"]
        .iter()
        .any(|suffix| name.ends_with(suffix))
}
