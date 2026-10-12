//! `js/one-var` (off by default; part of the `conventions` set)

use crate::prelude::*;

pub struct OneVar;

pub static ONE_VAR: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["conventions", "declarations"],
    remediation_minutes: 1,
    description: r#"Flags `var`, `let` and `const` statements that declare more than one variable.

**Why:** one name per statement is easier to read, to annotate with a type or a comment, to
change from `let` to `const`, and to move or delete in a diff. In a combined statement a `;`
typed instead of a `,` silently turns the remaining names into assignments to undeclared
variables (`var a = 1; b = 2`), and long lists hide which names are initialized.

**Fix:** write one declaration per variable.

```js
let width = 0, height = 0;    // flagged

let width = 0;                // compliant
let height = 0;
```

Not reported: the head of a `for` loop (`for (let i = 0, n = list.length; i < n; i++)`), where
several counters belong together, and `declare` statements.

Off by default: part of the opt-in `conventions` set.
"#,
    ..RuleMeta::new(
        "js/one-var",
        "js",
        "Each declaration statement should declare one variable",
    )
};

impl JsRule for OneVar {
    fn meta(&self) -> &'static RuleMeta {
        &ONE_VAR
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::VariableDeclaration]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::VariableDeclaration(decl) = node.kind() else {
            return;
        };
        if decl.declarations.len() < 2 || decl.declare {
            return;
        }
        if matches!(
            ctx.parent_kind(node),
            Some(
                AstKind::ForStatement(_) | AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
            )
        ) {
            return;
        }
        if ctx.in_ambient_context(node) {
            return;
        }
        let keyword = decl.kind.as_str();
        let span = Span::new(decl.span.start, decl.span.start + keyword.len() as u32);
        ctx.emit(
            span,
            format!(
                "Split this '{keyword}' into one statement per variable ({} declared here)",
                decl.declarations.len()
            ),
        );
    }
}
