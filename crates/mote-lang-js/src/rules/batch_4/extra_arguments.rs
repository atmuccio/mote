//! `js/extra-arguments`

use super::util;
use crate::prelude::*;

pub struct ExtraArguments;

pub static EXTRA_ARGUMENTS: RuleMeta = RuleMeta {
    tags: &["suspicious"],
    remediation_minutes: 5,
    description: r#"Flags calls that pass more arguments than the called function declares, when the
function is defined in the same file.

**Why:** JavaScript silently drops arguments that have no parameter, so the extra value never
reaches the function. Usually the call was written against an older or imagined signature, an
argument was put in the wrong call (`format(date, locale)` vs. `format(date), locale`), or the
function lost a parameter in a refactoring and its callers were not updated.

**Fix:** remove the extra argument, or add the missing parameter to the function.

```js
function greet(name) { return `Hello ${name}`; }

greet('Ada', 'Lovelace');   // flagged: 'greet' takes 1 parameter
greet('Ada');               // compliant
```

Only functions whose parameter list is known for sure are checked: function declarations and
`const` function expressions / arrow functions of the same file that are never reassigned or
overloaded, have no rest parameter, and never read `arguments`. Calls with spread arguments are
ignored.
"#,
    ..RuleMeta::new(
        "js/extra-arguments",
        "js",
        "Functions should not be called with more arguments than they declare",
    )
};

impl JsRule for ExtraArguments {
    fn meta(&self) -> &'static RuleMeta {
        &EXTRA_ARGUMENTS
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CallExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::CallExpression(call) = node.kind() else {
            return;
        };
        if call.arguments.is_empty() {
            return;
        }
        let Expression::Identifier(callee) = helpers::unwrap_expr(&call.callee) else {
            return;
        };
        let Some(symbol) = ctx.reference_symbol(callee) else {
            return;
        };
        let Some((params, function_span)) = callee_parameters(ctx, symbol) else {
            return;
        };
        if params.rest.is_some() || call.arguments.len() <= params.items.len() {
            return;
        }
        if call
            .arguments
            .iter()
            .any(|a| matches!(a, Argument::SpreadElement(_)))
        {
            return;
        }
        if !ctx.scoping().symbol_redeclarations(symbol).is_empty()
            || ctx.references_of(symbol).any(|r| r.is_write())
        {
            return;
        }
        // In a classic script another file may replace a top-level function.
        if ctx.scoping().symbol_scope_id(symbol) == ctx.scoping().root_scope_id()
            && !util::has_private_top_level(ctx)
        {
            return;
        }
        if util::uses_arguments_in(ctx, function_span) {
            return;
        }
        let Some(extra) = call.arguments.get(params.items.len()) else {
            return;
        };
        let declared = params.items.len();
        let noun = if declared == 1 {
            "parameter"
        } else {
            "parameters"
        };
        ctx.emit(
            extra.span(),
            format!(
                "Remove this extra argument: '{}' takes {declared} {noun} but is called with {}",
                callee.name,
                call.arguments.len()
            ),
        );
    }
}

/// Parameters and span of the function a symbol is bound to, when that is certain: a function
/// declaration with a body, or a `const` initialized with a function expression or arrow function.
fn callee_parameters<'a>(
    ctx: &Ctx<'a>,
    symbol: SymbolId,
) -> Option<(&'a FormalParameters<'a>, Span)> {
    let flags = ctx.symbol_flags(symbol);
    if flags.intersects(SymbolFlags::Ambient | SymbolFlags::Import) {
        return None;
    }
    let decl = ctx.symbol_declaration(symbol);
    match decl.kind() {
        AstKind::Function(f) => {
            if !f.is_declaration() || f.body.is_none() || f.declare {
                return None;
            }
            Some((&f.params, f.span))
        }
        AstKind::VariableDeclarator(d) => {
            if d.kind != VariableDeclarationKind::Const {
                return None;
            }
            if !matches!(d.id, BindingPattern::BindingIdentifier(_)) {
                return None;
            }
            // A typed variable (`const f: Handler = …`) must accept whatever its type allows.
            if d.type_annotation.is_some() {
                return None;
            }
            match d.init.as_ref()? {
                Expression::ArrowFunctionExpression(a) => Some((&a.params, a.span)),
                Expression::FunctionExpression(f) => Some((&f.params, f.span)),
                _ => None,
            }
        }
        _ => None,
    }
}
