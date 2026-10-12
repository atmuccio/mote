//! `js/no-multi-assign`

use crate::prelude::*;

pub struct NoMultiAssign;

pub static NO_MULTI_ASSIGN: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["readability"],
    remediation_minutes: 2,
    description: r#"Flags chained assignments such as `a = b = c` and `const a = b = c`.

**Why:** a chain hides an assignment in the middle of an expression. It is easy to misread
which variables are declared (in `const a = b = 0`, only `a` is; `b` is assigned — possibly
creating a global), and all targets silently share the same object.

**Fix:** write one assignment per statement. Each chain is reported once, at its second
assignment.

```js
a = b = 0;          // flagged
const x = y = [];   // flagged

a = 0;              // compliant
b = 0;
```
"#,
    ..RuleMeta::new(
        "js/no-multi-assign",
        "js",
        "Assignments should not be chained",
    )
};

/// If `node` is the value of an assignment, declarator, or class field, that holder's node.
fn holder<'a>(ctx: &Ctx<'a>, node: &AstNode<'a>) -> Option<&'a AstNode<'a>> {
    let parent = ctx.outer_parent(node)?;
    let span = node.span();
    let holds = match parent.kind() {
        AstKind::AssignmentExpression(a) => a.right.get_inner_expression().span() == span,
        AstKind::VariableDeclarator(d) => d
            .init
            .as_ref()
            .is_some_and(|i| i.get_inner_expression().span() == span),
        AstKind::PropertyDefinition(p) => p
            .value
            .as_ref()
            .is_some_and(|v| v.get_inner_expression().span() == span),
        _ => false,
    };
    holds.then_some(parent)
}

impl JsRule for NoMultiAssign {
    fn meta(&self) -> &'static RuleMeta {
        &NO_MULTI_ASSIGN
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::AssignmentExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::AssignmentExpression(assign) = node.kind() else {
            return;
        };
        let Some(outer) = holder(ctx, node) else {
            return;
        };
        // Report a chain once: skip when the holder is itself nested in a chain.
        if matches!(outer.kind(), AstKind::AssignmentExpression(_)) && holder(ctx, outer).is_some()
        {
            return;
        }
        ctx.emit(
            assign.span,
            "Split this chained assignment into separate statements",
        );
    }
}
