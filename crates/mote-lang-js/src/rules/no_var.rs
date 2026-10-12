//! `js/no-var`

use crate::prelude::*;

pub struct NoVar;

pub static NO_VAR: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["es2015", "style"],
    remediation_minutes: 2,
    description: r#"Flags `var` declarations.

**Why:** `var` is function-scoped and hoisted, so a variable declared inside a block leaks out of it,
can be used before its declaration line (as `undefined`), and is shared by every closure created in a
loop. `let` and `const` are block-scoped and fail loudly when used too early.

**Fix:** use `const` for bindings that are never reassigned and `let` for the others. Check that no
code relied on the variable being visible outside its block.

```js
var count = 0;          // flagged
for (var i = 0; i < n; i++) {} // flagged

const limit = 10;       // compliant
let total = 0;          // compliant
```

Ambient declarations (`declare var`, `declare global { var … }`) are not flagged: TypeScript
requires `var` there.
"#,
    ..RuleMeta::new("js/no-var", "js", "Use let or const instead of var")
};

impl JsRule for NoVar {
    fn meta(&self) -> &'static RuleMeta {
        &NO_VAR
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::VariableDeclaration]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::VariableDeclaration(decl) = node.kind() else {
            return;
        };
        if decl.kind != VariableDeclarationKind::Var || decl.declare || ctx.in_ambient_context(node)
        {
            return;
        }
        let keyword = Span::new(decl.span.start, decl.span.start + 3);
        let span = if ctx.text(keyword) == "var" {
            keyword
        } else {
            decl.span
        };
        ctx.emit(span, "Use 'let' or 'const' instead of 'var'");
    }
}
