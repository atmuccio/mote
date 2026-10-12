//! `js/no-shadow-restricted-names`

use crate::prelude::*;

pub struct NoShadowRestrictedNames;

pub static NO_SHADOW_RESTRICTED_NAMES: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["suspicious", "confusing"],
    remediation_minutes: 5,
    description: r#"Flags variables, parameters, functions, classes, catch parameters, and imports named
`undefined`, `NaN`, `Infinity`, `eval`, or `arguments`.

**Why:** every reader (and many tools) assumes these names mean the built-in values and functions.
A local binding with one of these names silently changes what the name means in its scope:
`undefined` can suddenly hold a value, `NaN` checks compare against something else, and `eval` or
`arguments` no longer do what they normally do. In strict mode code most of these bindings are
syntax errors.

**Fix:** rename the binding.

```js
function parse(input, undefined) {}  // flagged
const NaN = computeFallback();       // flagged
try { run(); } catch (eval) {}       // flagged

function parse(input, fallback) {}   // compliant
```

Not flagged: `var undefined;` without an initializer that is never assigned (it still holds
`undefined`), the classic `(function (undefined) { … })()` guard whose `undefined` parameter
never receives an argument, and TypeScript `declare` / ambient declarations.

Bindings named after words reserved in strict mode (`implements`, `interface`, `let`, `package`,
`private`, `protected`, `public`, `static`, `yield`) are flagged too: they only parse in sloppy
scripts, so the code breaks as soon as it becomes a module, a class body, or gains
`'use strict'`.

```js
var interface = load();              // flagged: a syntax error in strict code
```
"#,
    ..RuleMeta::new(
        "js/no-shadow-restricted-names",
        "js",
        "Built-in global names should not be redeclared",
    )
};

const RESTRICTED: &[&str] = &["undefined", "NaN", "Infinity", "eval", "arguments"];

/// Words reserved in strict mode code that sloppy scripts may still use as names.
const STRICT_RESERVED: &[&str] = &[
    "implements",
    "interface",
    "let",
    "package",
    "private",
    "protected",
    "public",
    "static",
    "yield",
];

impl JsRule for NoShadowRestrictedNames {
    fn meta(&self) -> &'static RuleMeta {
        &NO_SHADOW_RESTRICTED_NAMES
    }

    fn run_once<'a>(&self, ctx: &mut Ctx<'a>) {
        let scoping = ctx.scoping();
        let mut reports: Vec<(Span, &'a str)> = Vec::new();
        let mut reserved_reports: Vec<(Span, &'a str)> = Vec::new();
        for symbol in scoping.symbol_ids() {
            let name = scoping.symbol_name(symbol);
            let reserved = STRICT_RESERVED.contains(&name);
            if !RESTRICTED.contains(&name) && !reserved {
                continue;
            }
            let flags = scoping.symbol_flags(symbol);
            if !flags.intersects(
                SymbolFlags::Variable
                    | SymbolFlags::Function
                    | SymbolFlags::Class
                    | SymbolFlags::CatchVariable
                    | SymbolFlags::Import,
            ) || flags.intersects(SymbolFlags::Ambient | SymbolFlags::TypeImport)
            {
                continue;
            }
            let decl = ctx.symbol_declaration(symbol);
            if ctx.in_ambient_context(decl) || is_declare(ctx, decl) {
                continue;
            }
            if reserved {
                reserved_reports.push((scoping.symbol_span(symbol), name));
                continue;
            }
            match decl.kind() {
                // `var undefined;` keeps the value `undefined` as long as it is never assigned.
                AstKind::VariableDeclarator(d)
                    if d.init.is_none()
                        && name == "undefined"
                        && matches!(d.id, BindingPattern::BindingIdentifier(_))
                        && !ctx.references_of(symbol).any(|r| r.is_write()) => {}
                AstKind::FormalParameter(p)
                    if name == "undefined" && is_unfilled_iife_parameter(ctx, decl, p) => {}
                _ => reports.push((scoping.symbol_span(symbol), name)),
            }
        }
        for (span, name) in reports {
            ctx.emit(
                span,
                format!("Rename '{name}': it shadows the built-in global of the same name"),
            );
        }
        for (span, name) in reserved_reports {
            ctx.emit(
                span,
                format!("Rename '{name}': it is a reserved word in strict mode, so this code breaks in a module or with 'use strict'"),
            );
        }
    }
}

/// `declare var NaN: number`, `declare function eval(): void`, `declare class Infinity {}`.
fn is_declare(ctx: &Ctx<'_>, decl: &AstNode<'_>) -> bool {
    match decl.kind() {
        AstKind::VariableDeclarator(_) => {
            matches!(ctx.parent_kind(decl), Some(AstKind::VariableDeclaration(v)) if v.declare)
        }
        AstKind::Function(f) => f.declare || f.body.is_none(),
        AstKind::Class(c) => c.declare,
        // Parameters of type signatures, overloads, and bodiless declarations only describe types.
        AstKind::FormalParameter(_) => {
            let Some(params) = ctx.parent(decl) else {
                return true;
            };
            if matches!(params.kind(), AstKind::FormalParameters(p) if p.kind == FormalParameterKind::Signature)
            {
                return true;
            }
            match ctx.parent_kind(params) {
                Some(AstKind::Function(f)) => f.declare || f.body.is_none(),
                Some(AstKind::ArrowFunctionExpression(_)) => false,
                _ => true,
            }
        }
        _ => false,
    }
}

/// `(function (window, undefined) { … })(window)`: the `undefined` parameter of an immediately
/// invoked function that is called with fewer arguments than its position, so it really holds
/// `undefined` (a pre-ES5 guard against a reassigned global).
fn is_unfilled_iife_parameter(
    ctx: &Ctx<'_>,
    decl: &AstNode<'_>,
    param: &FormalParameter<'_>,
) -> bool {
    if !matches!(param.pattern, BindingPattern::BindingIdentifier(_)) {
        return false;
    }
    let Some(params_node) = ctx.parent(decl) else {
        return false;
    };
    let AstKind::FormalParameters(params) = params_node.kind() else {
        return false;
    };
    let Some(index) = params.items.iter().position(|p| p.span == param.span) else {
        return false;
    };
    let Some(function) = ctx.parent(params_node) else {
        return false;
    };
    let function_span = function.span();
    let Some(call_node) = ctx.outer_parent(function) else {
        return false;
    };
    let call = match call_node.kind() {
        AstKind::CallExpression(call) => call,
        // `(function () {}).call(this)` / `.apply(this, [...])` forms are not the idiom.
        _ => return false,
    };
    if helpers::unwrap_expr(&call.callee).span() != function_span {
        return false;
    }
    call.arguments.len() <= index
        && !call
            .arguments
            .iter()
            .any(|a| matches!(a, Argument::SpreadElement(_)))
}
