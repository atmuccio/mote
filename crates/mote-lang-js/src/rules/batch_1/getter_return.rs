//! `js/getter-return`

use super::util;
use crate::prelude::*;

pub struct GetterReturn;

pub static GETTER_RETURN: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["suspicious"],
    remediation_minutes: 5,
    description: r#"Flags getters (`get name() { … }` in classes and object literals, and `get` functions in
`Object.defineProperty` descriptors) that never return a value.

**Why:** reading the property calls the getter, and a getter without `return <value>` always
yields `undefined`. That is almost never the intent — usually the `return` was forgotten, or the
value was assigned to a variable and then dropped.

**Fix:** return the value. If the getter is meant to fail, make it `throw`.

```js
class Order {
  get total() {
    this.items.reduce((sum, i) => sum + i.price, 0); // flagged: result is discarded
  }
}

class Order {
  get total() {
    return this.items.reduce((sum, i) => sum + i.price, 0); // compliant
  }
}
```

Getters whose body always throws are not reported: a `throw`, a call of a same-file function that
always throws, or a test library's failing assertion that always throws (`assert.fail(…)`, also as
`t.assert.fail(…)`, `expect.fail(…)`, `should.fail(…)`, `expect.unreachable(…)`; not `t.fail(…)`,
which records a failure and returns). Also not reported: TypeScript accessor declarations without
a body, and getters that use a bare `return;` while declaring a return type that includes
`undefined` or `void`.
"#,
    ..RuleMeta::new("js/getter-return", "js", "Getters should return a value")
};

impl JsRule for GetterReturn {
    fn meta(&self) -> &'static RuleMeta {
        &GETTER_RETURN
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::MethodDefinition, AstType::ObjectProperty]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let (key_span, function) = match node.kind() {
            AstKind::MethodDefinition(m) if m.kind == MethodDefinitionKind::Get => {
                (m.key.span(), &*m.value)
            }
            AstKind::ObjectProperty(p) => {
                let function = match &p.value {
                    Expression::FunctionExpression(f) => f,
                    _ => return,
                };
                let descriptor_get = p.kind == PropertyKind::Init
                    && p.key.is_specific_static_name("get")
                    && !function.generator
                    && !function.r#async
                    && is_property_descriptor(ctx, node);
                if p.kind != PropertyKind::Get && !descriptor_get {
                    return;
                }
                (p.key.span(), &**function)
            }
            _ => return,
        };
        let Some(body) = &function.body else {
            return;
        };
        let (value, bare) = util::return_kinds(&body.statements);
        if value || (!bare && util::list_exits(&body.statements)) {
            return;
        }
        // `get x() { fail('not supported'); }` where `fail` always throws, or
        // `get x() { assert.fail('must not be read'); }`.
        if !bare
            && body
                .statements
                .iter()
                .any(|s| calls_never_returning(ctx, s, 0))
        {
            return;
        }
        // `get x(): T | undefined { return; }` returns `undefined` on purpose.
        if bare
            && function
                .return_type
                .as_ref()
                .is_some_and(|t| allows_undefined(&t.type_annotation))
        {
            return;
        }
        let name = util::excerpt(ctx.text(key_span));
        ctx.emit(key_span, format!("Return a value from the getter '{name}'"));
    }
}

/// Is this `get` property part of the descriptor object of `Object.defineProperty(o, k, {…})`,
/// `Reflect.defineProperty(o, k, {…})`, or a value of `Object.defineProperties(o, {k: {…}})` /
/// `Object.create(p, {k: {…}})`?
fn is_property_descriptor(ctx: &Ctx<'_>, property: &AstNode<'_>) -> bool {
    let Some(object) = ctx.parent(property) else {
        return false;
    };
    if !matches!(object.kind(), AstKind::ObjectExpression(_)) {
        return false;
    }
    let object_span = object.span();
    let Some(parent) = ctx.parent(object) else {
        return false;
    };
    let (call, arg_index) = match parent.kind() {
        AstKind::CallExpression(call) => (call, 2),
        AstKind::ObjectProperty(_) => {
            // `{ k: { get() {} } }` as the second argument of defineProperties / create.
            let Some(outer) = ctx.parent(parent) else {
                return false;
            };
            let Some(call_node) = ctx.parent(outer) else {
                return false;
            };
            let AstKind::CallExpression(call) = call_node.kind() else {
                return false;
            };
            let is_second = call
                .arguments
                .get(1)
                .is_some_and(|a| a.span() == outer.span());
            if !is_second {
                return false;
            }
            return matches!(
                helpers::call_member_names(call),
                Some(("Object", "defineProperties" | "create"))
            );
        }
        _ => return false,
    };
    call.arguments
        .get(arg_index)
        .is_some_and(|a| a.span() == object_span)
        && matches!(
            helpers::call_member_names(call),
            Some(("Object" | "Reflect", "defineProperty"))
        )
}

/// Is the statement a call that never returns normally: a test library's failing assertion
/// ([`is_failing_assertion`]), or a call (`fail(…);`) of a same-file function whose body always
/// throws, directly or through such a call of its own (followed a few levels deep)?
fn calls_never_returning(ctx: &Ctx<'_>, stmt: &Statement<'_>, depth: usize) -> bool {
    let Statement::ExpressionStatement(e) = stmt else {
        return false;
    };
    let Expression::CallExpression(call) = helpers::unwrap_expr(&e.expression) else {
        return false;
    };
    if is_failing_assertion(call) {
        return true;
    }
    let Expression::Identifier(callee) = helpers::unwrap_expr(&call.callee) else {
        return false;
    };
    if depth > 3 {
        return false;
    }
    let Some(symbol) = ctx.reference_symbol(callee) else {
        return false;
    };
    let statements = match ctx.symbol_declaration(symbol).kind() {
        AstKind::Function(f) if !f.r#async && !f.generator => match &f.body {
            Some(body) => &body.statements,
            None => return false,
        },
        AstKind::VariableDeclarator(d) if d.kind == VariableDeclarationKind::Const => {
            match d.init.as_ref().map(helpers::unwrap_expr) {
                Some(Expression::ArrowFunctionExpression(a)) if !a.r#async && !a.expression => {
                    &a.body.statements
                }
                Some(Expression::FunctionExpression(f)) if !f.r#async && !f.generator => {
                    match &f.body {
                        Some(body) => &body.statements,
                        None => return false,
                    }
                }
                _ => return false,
            }
        }
        _ => return false,
    };
    // An early `return` is a normal way out.
    if util::return_kinds(statements) != (false, false) {
        return false;
    }
    let throws = util::list_throws(statements)
        || statements
            .iter()
            .any(|s| calls_never_returning(ctx, s, depth + 1));
    // Checked last: linear in the number of uses of the function.
    throws
        && ctx.scoping().symbol_redeclarations(symbol).is_empty()
        && !ctx.references_of(symbol).any(|r| r.is_write())
}

/// A call of an assertion that always throws: `assert.fail(…)` (Node's `assert` and Chai's, also
/// reached as `t.assert.fail(…)` in `node:test` or `chai.assert.fail(…)`), `expect.fail(…)` and
/// `should.fail(…)` (Chai, should.js), `expect.unreachable(…)` (Vitest). Not `t.fail(…)`: tap,
/// tape and AVA record the failure and return.
fn is_failing_assertion(call: &CallExpression<'_>) -> bool {
    let Expression::StaticMemberExpression(member) = helpers::unwrap_expr(&call.callee) else {
        return false;
    };
    let method = member.property.name.as_str();
    match helpers::unwrap_expr(&member.object) {
        Expression::Identifier(object) => matches!(
            (object.name.as_str(), method),
            ("assert" | "expect" | "should", "fail") | ("expect", "unreachable")
        ),
        Expression::StaticMemberExpression(object) => {
            object.property.name == "assert" && method == "fail"
        }
        _ => false,
    }
}

/// `undefined`, `void`, `any`, `unknown`, or a union containing one of them.
fn allows_undefined(ty: &TSType<'_>) -> bool {
    match ty {
        TSType::TSUndefinedKeyword(_)
        | TSType::TSVoidKeyword(_)
        | TSType::TSAnyKeyword(_)
        | TSType::TSUnknownKeyword(_) => true,
        TSType::TSUnionType(u) => u.types.iter().any(allows_undefined),
        TSType::TSParenthesizedType(p) => allows_undefined(&p.type_annotation),
        _ => false,
    }
}
