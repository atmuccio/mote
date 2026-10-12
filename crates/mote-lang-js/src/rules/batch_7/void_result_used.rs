//! `js/void-result-used`

use super::util::is_function_node;
use crate::prelude::*;
use crate::rules::batch_6::util::descendants;

pub struct VoidResultUsed;

pub static VOID_RESULT_USED: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["suspicious"],
    remediation_minutes: 5,
    description: r#"Flags code that uses the result of calling a function of the same file that never returns a value:
`const user = loadUser(id)` when `loadUser` has no `return` with a value.

**Why:** such a call always evaluates to `undefined`. Storing it, passing it on, testing it or
reading a property of it means the author expected a value the function does not produce — a
missing `return`, a callback-based function used as if it returned its result, or a call to the
wrong function. The mistake surfaces later, far from its cause, as an `undefined` value or a
`TypeError`.

**Fix:** return the value from the function, or stop using the call's result.

```js
function findUser(id) {
  users.find((u) => u.id === id);     // the result is lost: no `return`
}

const user = findUser(7);             // flagged: always undefined
if (findUser(7)) {}                   // flagged

findUser(7);                          // compliant: result not used
```

Only functions declared in this file are checked (function declarations, and `const` holding an
arrow function or function expression with a block body), never reassigned, and neither `async`
nor generators. Functions that contain a `throw` are skipped (`return fail('…')` is a common way to
end a branch), and so are the positions where forwarding `undefined` is harmless: `return f()`,
arrow bodies, `void f()`, and arguments of test assertions.
"#,
    ..RuleMeta::new(
        "js/void-result-used",
        "js",
        "The result of a function that returns nothing should not be used",
    )
};

impl JsRule for VoidResultUsed {
    fn meta(&self) -> &'static RuleMeta {
        &VOID_RESULT_USED
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CallExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::CallExpression(call) = node.kind() else {
            return;
        };
        let Expression::Identifier(callee) = helpers::unwrap_expr(&call.callee) else {
            return;
        };
        let Some(usage) = usage(ctx, node) else {
            return;
        };
        let Some(symbol) = ctx.reference_symbol(callee) else {
            return;
        };
        let Some(function) = local_function(ctx, symbol) else {
            return;
        };
        // Each function body is scanned once per file, however often it is called.
        let id = function.id();
        let cached = ctx.state::<FxHashMap<NodeId, bool>>().get(&id).copied();
        let nothing = match cached {
            Some(nothing) => nothing,
            None => {
                let nothing = returns_nothing(ctx, function);
                ctx.state::<FxHashMap<NodeId, bool>>().insert(id, nothing);
                nothing
            }
        };
        if !nothing {
            return;
        }
        let name = callee.name.as_str();
        let declared = ctx.symbol_span(symbol);
        ctx.emit(
            call.span,
            format!(
                "'{name}()' never returns a value, so {usage}: return the value from '{name}' or stop using the result"
            ),
        )
        .secondary(declared, format!("'{name}' is declared here"));
    }
}

/// How the call's value is used, or `None` when it is discarded or only forwarded.
fn usage(ctx: &Ctx<'_>, call: &AstNode<'_>) -> Option<&'static str> {
    let call_span = call.span();
    let parent = ctx.outer_parent(call)?;
    let is = |e: &Expression<'_>| e.span() == call_span;
    let used = match parent.kind() {
        AstKind::VariableDeclarator(d) => d.init.as_ref().is_some_and(is),
        AstKind::AssignmentExpression(a) => is(&a.right),
        AstKind::StaticMemberExpression(m) if is(&m.object) => {
            return Some("reading a property of its result fails");
        }
        AstKind::ComputedMemberExpression(m) if is(&m.object) => {
            return Some("reading a property of its result fails");
        }
        AstKind::PrivateFieldExpression(m) if is(&m.object) => {
            return Some("reading a property of its result fails");
        }
        AstKind::IfStatement(s) if is(&s.test) => return Some("the condition is always false"),
        AstKind::WhileStatement(s) if is(&s.test) => return Some("the condition is always false"),
        AstKind::DoWhileStatement(s) if is(&s.test) => {
            return Some("the condition is always false");
        }
        AstKind::ConditionalExpression(c) if is(&c.test) => {
            return Some("the condition is always false");
        }
        AstKind::LogicalExpression(l) if is(&l.left) => true,
        AstKind::BinaryExpression(_) | AstKind::TemplateLiteral(_) => true,
        AstKind::UnaryExpression(u) => u.operator != UnaryOperator::Void,
        AstKind::CallExpression(outer) => {
            outer.arguments.iter().any(|a| a.span() == call_span) && !is_assertion(&outer.callee)
        }
        AstKind::NewExpression(outer) => outer.arguments.iter().any(|a| a.span() == call_span),
        AstKind::ObjectProperty(p) => is(&p.value),
        AstKind::ArrayExpression(_) => true,
        _ => false,
    };
    used.then_some("the value used here is always undefined")
}

/// `expect(…)`, `assert(…)`, `assert.x(…)`, `t.is(…)`-style assertion calls.
fn is_assertion(callee: &Expression<'_>) -> bool {
    match helpers::unwrap_expr(callee) {
        Expression::Identifier(id) => matches!(id.name.as_str(), "expect" | "assert" | "ok"),
        Expression::StaticMemberExpression(m) => match helpers::unwrap_expr(&m.object) {
            Expression::Identifier(id) => {
                matches!(id.name.as_str(), "assert" | "t" | "expect" | "chai")
            }
            _ => false,
        },
        _ => false,
    }
}

/// The function node behind a local, never-reassigned, non-async, non-generator function binding.
fn local_function<'a>(ctx: &Ctx<'a>, symbol: SymbolId) -> Option<&'a AstNode<'a>> {
    if ctx.symbol_flags(symbol).contains(SymbolFlags::Ambient)
        || !ctx.scoping().symbol_redeclarations(symbol).is_empty()
    {
        return None;
    }
    let declaration = ctx.symbol_declaration(symbol);
    let function = match declaration.kind() {
        AstKind::Function(f) => {
            if f.r#async
                || f.generator
                || f.body.is_none()
                || !void_or_none(f.return_type.as_deref())
            {
                return None;
            }
            declaration
        }
        AstKind::VariableDeclarator(d) => {
            if d.kind == VariableDeclarationKind::Var
                || !matches!(d.id, BindingPattern::BindingIdentifier(_))
                || d.type_annotation.is_some()
            {
                return None;
            }
            let init = d.init.as_ref()?;
            let ok = match helpers::unwrap_expr(init) {
                Expression::ArrowFunctionExpression(a) => {
                    !a.r#async && !a.expression && void_or_none(a.return_type.as_deref())
                }
                Expression::FunctionExpression(f) => {
                    !f.r#async && !f.generator && void_or_none(f.return_type.as_deref())
                }
                _ => false,
            };
            if !ok {
                return None;
            }
            // The declarator's first function child is the initializer.
            descendants(ctx, declaration).find(|n| is_function_node(n.kind()))?
        }
        _ => return None,
    };
    // Checked last: linear in the number of uses.
    if ctx.references_of(symbol).any(|r| r.is_write()) {
        return None;
    }
    Some(function)
}

/// The function body has no `return <value>` and no `throw` of its own.
fn returns_nothing(ctx: &Ctx<'_>, function: &AstNode<'_>) -> bool {
    let mut skip_until = 0;
    for inner in descendants(ctx, function) {
        let span = inner.span();
        if span.start < skip_until {
            continue;
        }
        match inner.kind() {
            // Nested functions and classes return for themselves.
            kind if is_function_node(kind) => skip_until = span.end,
            AstKind::Class(_) => skip_until = span.end,
            AstKind::ReturnStatement(r) if r.argument.is_some() => return false,
            AstKind::ThrowStatement(_) => return false,
            _ => {}
        }
    }
    true
}

/// No return type annotation, or `: void`.
fn void_or_none(annotation: Option<&TSTypeAnnotation<'_>>) -> bool {
    annotation.is_none_or(|a| matches!(a.type_annotation, TSType::TSVoidKeyword(_)))
}
