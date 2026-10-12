//! `js/no-this-before-super`

use super::constructor_super::derived_constructor;
use super::util;
use crate::prelude::*;

pub struct NoThisBeforeSuper;

pub static NO_THIS_BEFORE_SUPER: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["classes", "suspicious"],
    remediation_minutes: 5,
    description: r#"Flags `this` or `super.member` used in the constructor of a derived class before `super()` has
been called.

**Why:** in a class that extends another, the instance is created by the base class constructor.
Until `super()` returns, `this` is uninitialized and any access to it — reading a field, calling a
method, even passing `this` as an argument to `super(…)` itself — throws a `ReferenceError`.

**Fix:** move the `super(…)` call before the first use of `this`. Values that must be computed
first can go into local variables or into the arguments of `super(…)`.

```js
class Button extends Widget {
  constructor(label) {
    this.label = label;        // flagged: runs before super()
    super(label);
  }
}

class Button extends Widget {
  constructor(label) {
    super(label);
    this.label = label;        // compliant
  }
}
```

Only uses that come before the first `super()` call in the source text are reported, and only in
the constructor itself: an arrow function created before `super()` may use `this` when it is called
later, and a nested `function` or class has its own `this`.
"#,
    ..RuleMeta::new(
        "js/no-this-before-super",
        "js",
        "'this' should not be used before super() in a derived constructor",
    )
};

impl JsRule for NoThisBeforeSuper {
    fn meta(&self) -> &'static RuleMeta {
        &NO_THIS_BEFORE_SUPER
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.file_contains(AstType::Super)
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::MethodDefinition]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let Some((_, function)) = derived_constructor(ctx, node) else {
            return;
        };
        // The end of the first `super()` call of the constructor itself: `this` inside its
        // arguments is used before the call completes.
        let first_super_end =
            util::descendants(ctx, function).find_map(|inner| match inner.kind() {
                AstKind::CallExpression(call)
                    if matches!(call.callee, Expression::Super(_))
                        && util::this_owner(ctx, inner).map(AstNode::id) == Some(function.id()) =>
                {
                    Some(call.span.end)
                }
                _ => None,
            });
        let Some(limit) = first_super_end else {
            return;
        };
        let mut reports = Vec::new();
        for inner in util::descendants(ctx, function) {
            if inner.span().start >= limit {
                break;
            }
            let what = match inner.kind() {
                AstKind::ThisExpression(_) => "'this'",
                AstKind::Super(_) if !is_super_call(ctx, inner) => "'super'",
                _ => continue,
            };
            // Only the constructor's own code: not arrows (called later), not nested functions or
            // classes (their own `this`).
            let owner = ctx.ancestors(inner).find(|a| {
                matches!(
                    a.kind(),
                    AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) | AstKind::Class(_)
                )
            });
            if owner.map(AstNode::id) == Some(function.id()) {
                reports.push((inner.span(), what));
            }
        }
        for (span, what) in reports {
            ctx.emit(
                span,
                format!("Call 'super()' before using {what}: it is not initialized yet in a derived class constructor"),
            );
        }
    }
}

fn is_super_call(ctx: &Ctx<'_>, node: &AstNode<'_>) -> bool {
    matches!(ctx.parent_kind(node), Some(AstKind::CallExpression(call)) if call.callee.span() == node.span())
}
