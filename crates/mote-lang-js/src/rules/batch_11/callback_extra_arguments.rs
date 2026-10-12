//! `js/callback-extra-arguments`

use crate::prelude::*;

pub struct CallbackExtraArguments;

pub static CALLBACK_EXTRA_ARGUMENTS: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["suspicious", "callback"],
    remediation_minutes: 2,
    params: &[ParamSpec {
        name: "local_functions",
        default: ParamDefault::Bool(false),
        description: "Also report functions of the same file whose second parameter is optional (has a default value or `?`) and is not named like an index.",
    }],
    description: r#"Flags a function passed by name to an array method (`map`, `flatMap`, `filter`, `some`,
`every`, `find`, `findIndex`, `findLast`, `findLastIndex`, or the mapping function of
`Array.from`) when the function gives a meaning to the extra arguments the method passes: the
element's index and the array. The functions checked are those whose second parameter changes
their result:

* `parseInt` and `Number.parseInt` take the index as the radix: `['1', '2', '3'].map(parseInt)`
  is `[1, NaN, NaN]` (radix 0 means 10, radix 1 is invalid, `'3'` is not a digit in base 2);
* `String.fromCharCode` and `String.fromCodePoint` turn the index and the array into extra
  characters (or throw a `RangeError`);
* `Array.from` takes the index as its mapping function and throws a `TypeError`.

**Why:** the array methods call their callback with three arguments, not one. A function written
to be called with one argument and an optional second one silently uses the index for that
second one. The code looks right, works for the first element, and goes wrong from the second.

**Fix:** pass an arrow function that forwards only the element (and an explicit radix).

```js
const ids = parts.map(parseInt);                   // flagged
const ids = parts.map((part) => parseInt(part, 10)); // compliant

const codes = values.map(Number);                  // compliant: Number takes one argument
```

Functions that take only one argument (`Number`, `parseFloat`, `Boolean`, `String`, `Math.abs`)
are not reported, nor is `forEach` (the result is not used). With the `local_functions` parameter,
the rule also reports functions of the same file whose second parameter is optional
(`function format(value, digits = 2)` passed as `values.map(format)`), unless that parameter is
named like an index (`i`, `index`, `idx`, `key`, …) or the function reads
`arguments`; `forEach` counts there.
"#,
    ..RuleMeta::new(
        "js/callback-extra-arguments",
        "js",
        "Functions passed to array methods should not receive the index as an argument they misread",
    )
};

/// Array methods that call their callback with `(element, index, array)` and use its result.
const RESULT_METHODS: &[&str] = &[
    "map",
    "flatMap",
    "filter",
    "some",
    "every",
    "find",
    "findIndex",
    "findLast",
    "findLastIndex",
];

const CALL_NAMES: &[&str] = &[
    "map",
    "flatMap",
    "filter",
    "some",
    "every",
    "find",
    "findIndex",
    "findLast",
    "findLastIndex",
    "forEach",
    "from",
];

/// Names of a second parameter that expects the index.
const INDEX_NAMES: &[&str] = &[
    "i", "j", "k", "n", "ix", "idx", "index", "key", "pos", "position",
];

impl JsRule for CallbackExtraArguments {
    fn meta(&self) -> &'static RuleMeta {
        &CALLBACK_EXTRA_ARGUMENTS
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CallExpression]
    }

    fn callee_names(&self) -> &'static [&'static str] {
        CALL_NAMES
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::CallExpression(call) = node.kind() else {
            return;
        };
        let Some(member) = call.callee.get_inner_expression().as_member_expression() else {
            return;
        };
        let Some(method) = member.static_property_name() else {
            return;
        };
        // The callback: the first argument of an array method (a second one is `thisArg`), the
        // second argument of `Array.from`.
        let callback = match method {
            "from" => {
                if !ctx.is_global(member.object(), "Array") || call.arguments.len() < 2 {
                    return;
                }
                call.arguments.get(1)
            }
            m if RESULT_METHODS.contains(&m) || m == "forEach" => {
                if call.arguments.len() > 2 {
                    return;
                }
                call.arguments.first()
            }
            _ => return,
        };
        let Some(callback) = callback.and_then(Argument::as_expression) else {
            return;
        };
        let callback = helpers::unwrap_expr(callback);
        let method = if method == "from" {
            "Array.from"
        } else {
            method
        };
        // `forEach` ignores the result: what a built-in computes from the index is lost anyway.
        if method != "forEach" {
            if let Some(problem) = builtin_problem(ctx, callback) {
                let name = ctx.text(callback.span());
                ctx.emit(
                    callback.span(),
                    format!(
                        "Pass an arrow function that calls '{name}' with the element only: '{method}' also passes the index, which {problem}"
                    ),
                );
                return;
            }
        }
        let Expression::Identifier(id) = callback else {
            return;
        };
        if !ctx.param_bool("local_functions") {
            return;
        }
        let Some(param) = misread_parameter(ctx, id) else {
            return;
        };
        ctx.emit(
            callback.span(),
            format!(
                "Pass an arrow function that calls '{}' with the element only: '{method}' passes the index as its optional parameter '{param}'",
                id.name
            ),
        );
    }
}

/// What a built-in function does with the index it receives as its second argument, when that
/// changes its result.
fn builtin_problem(ctx: &Ctx<'_>, callback: &Expression<'_>) -> Option<&'static str> {
    match callback {
        Expression::Identifier(id) => (id.name == "parseInt" && ctx.is_global_reference(id))
            .then_some("it reads as the radix"),
        Expression::StaticMemberExpression(m) => match (m.property.name.as_str(), &m.object) {
            ("parseInt", object) if ctx.is_global(object, "Number") => {
                Some("it reads as the radix")
            }
            ("fromCharCode" | "fromCodePoint", object) if ctx.is_global(object, "String") => {
                Some("it turns into an extra character, as does the array")
            }
            ("from", object) if ctx.is_global(object, "Array") => {
                Some("it takes for a mapping function and throws")
            }
            _ => None,
        },
        _ => None,
    }
}

/// The optional second parameter of a same-file function, which receives the index when the
/// function is passed as a callback.
fn misread_parameter<'a>(ctx: &Ctx<'a>, id: &IdentifierReference<'a>) -> Option<&'a str> {
    let symbol = ctx.reference_symbol(id)?;
    let (params, function_span) = function_parameters(ctx, symbol)?;
    let param = params.items.get(1)?;
    if param.initializer.is_none() && !param.optional {
        return None;
    }
    let BindingPattern::BindingIdentifier(name) = &param.pattern else {
        return None;
    };
    let name = name.name.as_str();
    // A parameter named like an index is meant to receive it.
    if INDEX_NAMES.contains(&name) || name.starts_with('_') {
        return None;
    }
    if !ctx.scoping().symbol_redeclarations(symbol).is_empty()
        || ctx.references_of(symbol).any(|r| r.is_write())
    {
        return None;
    }
    // In a classic script another file may replace a top-level function.
    if ctx.scoping().symbol_scope_id(symbol) == ctx.scoping().root_scope_id()
        && !has_private_top_level(ctx)
    {
        return None;
    }
    if uses_arguments_in(ctx, function_span) {
        return None;
    }
    Some(name)
}

/// Parameters and span of the function a symbol is bound to, when that is certain: a function
/// declaration with a body, or a `const` initialized with a function expression or arrow
/// function (without a type annotation, which may allow other functions).
fn function_parameters<'a>(
    ctx: &Ctx<'a>,
    symbol: SymbolId,
) -> Option<(&'a FormalParameters<'a>, Span)> {
    if ctx
        .symbol_flags(symbol)
        .intersects(SymbolFlags::Ambient | SymbolFlags::Import)
    {
        return None;
    }
    match ctx.symbol_declaration(symbol).kind() {
        AstKind::Function(f) => {
            if !f.is_declaration() || f.body.is_none() || f.declare {
                return None;
            }
            Some((&f.params, f.span))
        }
        AstKind::VariableDeclarator(d) => {
            if d.kind != VariableDeclarationKind::Const
                || !matches!(d.id, BindingPattern::BindingIdentifier(_))
                || d.type_annotation.is_some()
            {
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

/// Is the file a module (or CommonJS), so that its top-level names are private to it?
fn has_private_top_level(ctx: &Ctx<'_>) -> bool {
    let unresolved = ctx.scoping().root_unresolved_references();
    ctx.is_module()
        || ctx.source_type().is_commonjs()
        || unresolved.contains_key("require")
        || unresolved.contains_key("module")
        || unresolved.contains_key("exports")
}

/// Does code inside `span` read `arguments`?
fn uses_arguments_in(ctx: &Ctx<'_>, span: Span) -> bool {
    let scoping = ctx.scoping();
    scoping
        .root_unresolved_references()
        .get("arguments")
        .is_some_and(|ids| {
            ids.iter().any(|&id| {
                let at = ctx.node(scoping.get_reference(id).node_id()).span().start;
                span.start <= at && at < span.end
            })
        })
}
