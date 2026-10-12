//! `js/async-in-constructor`

use super::util;
use crate::prelude::*;
use std::cell::Cell;

pub struct AsyncInConstructor;

pub static ASYNC_IN_CONSTRUCTOR: RuleMeta = RuleMeta {
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["async", "design"],
    remediation_minutes: 20,
    description: r#"Flags constructors that start asynchronous work which sets up the object's own state, and drop
the promise: a call to an `async` method of the same class, an `async` arrow function called on the
spot, or a `.then(…)` callback, that assigns a field of `this` once the constructor has returned.

**Why:** a constructor cannot wait. The caller receives the object while the work is still
running, so its first method calls see fields that are not set yet, and nothing tells the caller
when the object becomes usable. A failure of the dropped promise is not attached to anything: it
surfaces as an unhandled rejection far from the code that created the object, or is lost.

**Fix:** move the asynchronous part into a static factory (`static async create()`) that awaits it
before handing out the instance, or into an explicit `init()` the caller awaits. If the object must
start the work itself, keep the promise in a field (`this.ready = this.load()`) so callers can wait
for it and failures stay attached.

```js
class Catalog {
  constructor(api) {
    this.api = api;
    this.load();                              // flagged: items arrive later
  }
  async load() { this.items = await this.api.items(); }
  get size() { return this.items.length; }
}

class Catalog {
  static async create(api) {                  // compliant
    const catalog = new Catalog(api);
    await catalog.load();
    return catalog;
  }
  constructor(api) { this.api = api; }
  async load() { this.items = await this.api.items(); }
  get size() { return this.items.length; }
}
```

Only work that leaves the object half initialized is reported: the async method (or the `then`
callback) assigns a field of `this` after its first `await`, the class reads that field elsewhere,
and nothing prepares the object for the field being missing (no initializer, no optional or
`undefined`-typed declaration, no assignment in the constructor, no parameter property): a class
that starts with `ready = false` and checks it has planned for the wait. Background work that does not set up the object's own state (refreshing something
else, reporting telemetry) is left to `js/floating-promise`. Not reported either: promises kept in
a variable or field, chains ending in `.catch(…)` or given a rejection handler, calls marked as
deliberately detached with `void`, and work started in callbacks created by the constructor (event
listeners, timers), which run later.
"#,
    ..RuleMeta::new(
        "js/async-in-constructor",
        "js",
        "Constructors should not start asynchronous work they cannot wait for",
    )
};

impl JsRule for AsyncInConstructor {
    fn meta(&self) -> &'static RuleMeta {
        &ASYNC_IN_CONSTRUCTOR
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::MethodDefinition]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::MethodDefinition(ctor) = node.kind() else {
            return;
        };
        if ctor.kind != MethodDefinitionKind::Constructor {
            return;
        }
        let Some(body) = &ctor.value.body else {
            return;
        };
        if body.statements.is_empty() {
            return;
        }
        let Some(class_body_node) = ctx.parent(node) else {
            return;
        };
        let AstKind::ClassBody(class_body) = class_body_node.kind() else {
            return;
        };
        let ctor_function = ctx.node(ctor.value.node_id.get());
        let mut reads: Option<Vec<(&str, u32)>> = None;
        for inner in util::own_nodes(ctx, ctor_function) {
            let AstKind::ExpressionStatement(stmt) = inner.kind() else {
                continue;
            };
            let Expression::CallExpression(call) = helpers::unwrap_expr(&stmt.expression) else {
                continue;
            };
            let Some((what, work, whole)) = started_work(ctx, call, class_body) else {
                continue;
            };
            let written = async_this_writes(ctx, work, whole);
            if written.is_empty() {
                continue;
            }
            let reads = reads.get_or_insert_with(|| this_reads(ctx, class_body_node));
            let work_span = work.span();
            let Some(field) = written.iter().find(|f| {
                !prepared_field(ctx, class_body, ctor_function, f)
                    && reads.iter().any(|(name, at)| {
                        name == *f && !(work_span.start <= *at && *at < work_span.end)
                    })
            }) else {
                continue;
            };
            ctx.emit(
                util::first_line(ctx, call.span),
                format!(
                    "Do not start {what} in the constructor: it sets 'this.{field}' later, so callers get a half-initialized object; use a static async factory or an awaited init()"
                ),
            );
        }
    }
}

/// The asynchronous work a discarded call starts: its description, the function doing it, and
/// whether all of that function runs later (a `then` callback) or only what follows its first
/// `await`.
fn started_work<'a>(
    ctx: &Ctx<'a>,
    call: &CallExpression<'a>,
    class_body: &ClassBody<'a>,
) -> Option<(String, &'a AstNode<'a>, bool)> {
    let node_of = |id: &Cell<NodeId>| ctx.node(id.get());
    match helpers::unwrap_expr(&call.callee) {
        Expression::StaticMemberExpression(m) => {
            let name = m.property.name.as_str();
            match helpers::unwrap_expr(&m.object) {
                // `this.load()`
                Expression::ThisExpression(_) => {
                    let function = async_method(class_body, name)?;
                    Some((
                        format!("the async method '{name}'"),
                        node_of(function),
                        false,
                    ))
                }
                // `….then(() => { this.x = … })`
                _ if name == "then" && call.arguments.len() == 1 => {
                    match helpers::unwrap_expr(call.arguments.first()?.as_expression()?) {
                        Expression::ArrowFunctionExpression(a) => {
                            Some(("a promise chain".to_string(), node_of(&a.node_id), true))
                        }
                        _ => None,
                    }
                }
                _ => None,
            }
        }
        // `(async () => { … })()`
        Expression::ArrowFunctionExpression(a) if a.r#async => {
            Some(("an async function".to_string(), node_of(&a.node_id), false))
        }
        _ => None,
    }
}

/// The function node id of an `async` instance method (or `async` arrow property) named `name`.
fn async_method<'b>(class_body: &'b ClassBody<'_>, name: &str) -> Option<&'b Cell<NodeId>> {
    class_body.body.iter().find_map(|element| match element {
        ClassElement::MethodDefinition(m)
            if !m.r#static
                && m.kind == MethodDefinitionKind::Method
                && m.value.r#async
                && !m.value.generator
                && m.value.body.is_some()
                && m.key.is_specific_static_name(name) =>
        {
            Some(&m.value.node_id)
        }
        ClassElement::PropertyDefinition(p)
            if !p.r#static && p.key.is_specific_static_name(name) =>
        {
            match &p.value {
                Some(Expression::ArrowFunctionExpression(a)) if a.r#async => Some(&a.node_id),
                _ => None,
            }
        }
        _ => None,
    })
}

/// Fields of `this` that the function assigns after its first `await` (anywhere when `whole`).
fn async_this_writes<'a>(ctx: &Ctx<'a>, function: &AstNode<'a>, whole: bool) -> Vec<&'a str> {
    let nodes = util::own_nodes(ctx, function);
    let first_await = if whole {
        Some(0)
    } else {
        nodes.iter().find_map(|n| match n.kind() {
            AstKind::AwaitExpression(a) => Some(a.span.start),
            _ => None,
        })
    };
    let Some(first_await) = first_await else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for node in nodes {
        let AstKind::AssignmentExpression(assign) = node.kind() else {
            continue;
        };
        // `this.x = await y` completes after the await; `this.x = 1; await y` before it.
        if assign.span.end <= first_await {
            continue;
        }
        if let Some(MemberExpression::StaticMemberExpression(m)) =
            assign.left.as_member_expression()
        {
            if matches!(m.object, Expression::ThisExpression(_)) {
                out.push(m.property.name.as_str());
            }
        }
    }
    out
}

/// Does the class prepare for the field being missing or already have it: a field declaration
/// with an initializer, marked optional or typed to allow `undefined`/`null`, a parameter
/// property, or an assignment in the constructor?
fn prepared_field(
    ctx: &Ctx<'_>,
    class_body: &ClassBody<'_>,
    ctor: &AstNode<'_>,
    name: &str,
) -> bool {
    for element in &class_body.body {
        match element {
            ClassElement::PropertyDefinition(p)
                if !p.r#static && p.key.is_specific_static_name(name) =>
            {
                let loose_type = p.type_annotation.as_ref().is_some_and(|t| {
                    ctx.text(t.span)
                        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                        .any(|w| matches!(w, "undefined" | "null" | "any" | "unknown" | "void"))
                });
                if p.value.is_some() || p.optional || loose_type {
                    return true;
                }
            }
            ClassElement::MethodDefinition(m) if m.kind == MethodDefinitionKind::Constructor => {
                let is_param_property = m.value.params.items.iter().any(|param| {
                    param.accessibility.is_some() || param.readonly
                } && matches!(&param.pattern, BindingPattern::BindingIdentifier(b) if b.name == name));
                if is_param_property {
                    return true;
                }
            }
            _ => {}
        }
    }
    util::own_nodes(ctx, ctor).iter().any(|n| match n.kind() {
        AstKind::AssignmentExpression(a) => matches!(
            a.left.as_member_expression(),
            Some(MemberExpression::StaticMemberExpression(m))
                if matches!(m.object, Expression::ThisExpression(_)) && m.property.name == name
        ),
        _ => false,
    })
}

/// `this.x` reads in the class body (not plain assignment targets), with their positions.
fn this_reads<'a>(ctx: &Ctx<'a>, class_body: &AstNode<'a>) -> Vec<(&'a str, u32)> {
    let mut out = Vec::new();
    for node in util::descendants(ctx, class_body) {
        let AstKind::StaticMemberExpression(m) = node.kind() else {
            continue;
        };
        if !matches!(m.object, Expression::ThisExpression(_)) {
            continue;
        }
        if let Some(AstKind::AssignmentExpression(a)) = ctx.parent_kind(node) {
            if a.operator == AssignmentOperator::Assign && a.left.span() == m.span {
                continue;
            }
        }
        out.push((m.property.name.as_str(), m.span.start));
    }
    out
}
