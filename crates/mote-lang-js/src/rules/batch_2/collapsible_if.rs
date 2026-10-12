//! `js/collapsible-if`

use crate::prelude::*;

pub struct CollapsibleIf;

pub static COLLAPSIBLE_IF: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["readability"],
    remediation_minutes: 2,
    description: r#"Flags an `if` statement without `else` whose body is nothing but another `if`
without `else`.

**Why:** two nested conditions that both have to hold are one condition. The extra level of
nesting makes the code harder to read and pushes the real work further to the right.

**Fix:** combine the conditions with `&&`. Nothing is reported when either `if` has an `else`
(merging would change which branch runs), or when a comment precedes the inner `if` (it usually
documents the outer condition).

```js
if (user) {
  if (user.isAdmin) {        // flagged
    grant();
  }
}

if (user && user.isAdmin) {  // compliant
  grant();
}
```
"#,
    ..RuleMeta::new(
        "js/collapsible-if",
        "js",
        "Nested 'if' statements without 'else' should be merged",
    )
};

impl JsRule for CollapsibleIf {
    fn meta(&self) -> &'static RuleMeta {
        &COLLAPSIBLE_IF
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::IfStatement]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::IfStatement(outer) = node.kind() else {
            return;
        };
        if outer.alternate.is_some() {
            return;
        }
        let (inner, block_start) = match &outer.consequent {
            Statement::IfStatement(inner) => (inner, None),
            Statement::BlockStatement(block) => match block.body.as_slice() {
                [Statement::IfStatement(inner)] => (inner, Some(block.span.start)),
                _ => return,
            },
            _ => return,
        };
        if inner.alternate.is_some() {
            return;
        }
        if let Some(start) = block_start {
            if ctx.has_comments_in(Span::new(start, inner.span.start)) {
                return;
            }
        }
        ctx.emit(
            Span::new(inner.span.start, inner.span.start + 2),
            "Merge this 'if' with the enclosing one using '&&'",
        );
    }
}
