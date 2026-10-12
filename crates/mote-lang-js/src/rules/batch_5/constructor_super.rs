//! `js/constructor-super`

use super::util;
use crate::prelude::*;

pub struct ConstructorSuper;

pub static CONSTRUCTOR_SUPER: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["classes", "suspicious"],
    remediation_minutes: 5,
    description: r#"Flags the constructor of a derived class (`class A extends B`) that never calls `super()`.

**Why:** in a class that extends another, `this` does not exist until `super()` has run. A
constructor that never calls it throws a `ReferenceError` as soon as the class is instantiated —
even if the constructor itself never touches `this`, because the object has to be returned when the
constructor ends. This typically happens when a class gains an `extends` clause later, or when the
`super()` call is lost in a refactoring.

**Fix:** call `super(…)` (with the arguments the base class expects) before using `this`, or
remove the `extends` clause if the class does not need a base class.

```js
class Admin extends User {
  constructor(name) {           // flagged: never calls super()
    this.name = name;
  }
}

class Admin extends User {
  constructor(name) {
    super(name);                // compliant
    this.role = 'admin';
  }
}
```

Not reported: constructors that return a value of their own (`return Object.create(…)`, which is
allowed without `super()`), classes that extend `null`, and constructor signatures without a body
(TypeScript overloads and declarations). A `super()` call inside an arrow function in the
constructor counts.

Also flagged: a second `super()` call that runs unconditionally after a first one (both are
statements of the constructor body, outside any `if`, loop, `try`, or callback). The object is
already initialized by the first call, so the second throws a `ReferenceError`.

```js
class Admin extends User {
  constructor(name) {
    super(name);
    this.role = 'admin';
    super(name);                // flagged: throws, `this` is already initialized
  }
}
```
"#,
    ..RuleMeta::new(
        "js/constructor-super",
        "js",
        "Constructors of derived classes should call super()",
    )
};

impl JsRule for ConstructorSuper {
    fn meta(&self) -> &'static RuleMeta {
        &CONSTRUCTOR_SUPER
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.file_contains(AstType::Class)
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::MethodDefinition]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let Some((method, function)) = derived_constructor(ctx, node) else {
            return;
        };
        let mut calls_super = false;
        let mut first_unconditional: Option<Span> = None;
        for inner in util::descendants(ctx, function) {
            match inner.kind() {
                AstKind::CallExpression(call) if matches!(call.callee, Expression::Super(_)) => {
                    if util::this_owner(ctx, inner).map(AstNode::id) == Some(function.id()) {
                        calls_super = true;
                        if !runs_unconditionally(ctx, inner, function) {
                            continue;
                        }
                        match first_unconditional {
                            None => first_unconditional = Some(call.span),
                            Some(first) => {
                                ctx.emit(
                                    call.span,
                                    "Remove this second 'super()' call: the object is already initialized, so calling it again throws",
                                )
                                .secondary(first, "First call");
                                return;
                            }
                        }
                    }
                }
                AstKind::ReturnStatement(ret)
                    if ret.argument.is_some()
                    // Returning an object of its own is allowed without `super()`.
                    && util::this_owner(ctx, inner).map(AstNode::id) == Some(function.id()) =>
                {
                    return;
                }
                _ => {}
            }
        }
        if !calls_super {
            ctx.emit(
                method.key.span(),
                "Call 'super()' in this constructor: the class extends another class, so creating an instance throws without it",
            );
        }
    }
}

/// Is the `super()` call a statement of the constructor body itself (possibly inside plain nested
/// blocks), so that it runs whenever the constructor runs?
fn runs_unconditionally(ctx: &Ctx<'_>, call: &AstNode<'_>, function: &AstNode<'_>) -> bool {
    let Some(statement) = ctx.parent(call) else {
        return false;
    };
    if !matches!(statement.kind(), AstKind::ExpressionStatement(_)) {
        return false;
    }
    for ancestor in ctx.ancestors(statement) {
        match ancestor.kind() {
            AstKind::BlockStatement(_) => {}
            AstKind::FunctionBody(_) => {
                return ctx.parent(ancestor).map(AstNode::id) == Some(function.id());
            }
            _ => return false,
        }
    }
    false
}

/// The constructor method definition and its function node when `node` is the constructor (with a
/// body) of a class whose `extends` clause is not `null`.
pub(super) fn derived_constructor<'a>(
    ctx: &Ctx<'a>,
    node: &AstNode<'a>,
) -> Option<(&'a MethodDefinition<'a>, &'a AstNode<'a>)> {
    let AstKind::MethodDefinition(method) = node.kind() else {
        return None;
    };
    if method.kind != MethodDefinitionKind::Constructor || method.value.body.is_none() {
        return None;
    }
    let class = ctx
        .ancestors(node)
        .find(|a| matches!(a.kind(), AstKind::Class(_)))?;
    let AstKind::Class(class) = class.kind() else {
        return None;
    };
    let super_class = class.super_class.as_ref()?;
    if matches!(
        helpers::unwrap_expr(super_class),
        Expression::NullLiteral(_)
    ) || class.declare
        || ctx.in_ambient_context(node)
    {
        return None;
    }
    let function = ctx.node(method.value.node_id.get());
    Some((method, function))
}
