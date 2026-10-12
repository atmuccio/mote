//! `js/no-unassigned-vars`

use super::util;
use crate::prelude::*;

pub struct NoUnassignedVars;

pub static NO_UNASSIGNED_VARS: RuleMeta = RuleMeta {
    severity: Severity::Medium,
    tags: &["suspicious", "unused"],
    remediation_minutes: 5,
    description: r#"Flags `let` and `var` variables that are declared without a value, never assigned
anywhere, and yet read.

**Why:** such a variable is always `undefined`, so every read of it yields the same useless value.
Usually an assignment was forgotten or went to another variable by mistake; sometimes the variable
is simply left over and its reads are dead code.

**Fix:** assign the intended value, or remove the variable and use `undefined` (or the right
variable) directly.

```js
let timeout;
setTimeout(run, timeout);    // flagged at the declaration: always undefined

let timeout = 1000;          // compliant
setTimeout(run, timeout);
```

Not flagged: variables assigned anywhere in the file (including in callbacks or destructuring),
`for…in` / `for…of` heads, exported, `declare`d, and ambient variables, variables typed as only
`undefined` / `null` / `void` (deliberately empty), and top-level variables of
classic scripts (another script may assign them).
"#,
    ..RuleMeta::new(
        "js/no-unassigned-vars",
        "js",
        "Variables that are read should be assigned",
    )
};

impl JsRule for NoUnassignedVars {
    fn meta(&self) -> &'static RuleMeta {
        &NO_UNASSIGNED_VARS
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::VariableDeclaration]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        // `with` and direct `eval` can assign any variable by name.
        !ctx.file_contains(AstType::WithStatement) && !util::uses_global(ctx, "eval")
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::VariableDeclaration(decl) = node.kind() else {
            return;
        };
        if !matches!(
            decl.kind,
            VariableDeclarationKind::Let | VariableDeclarationKind::Var
        ) || decl.declare
            || decl.declarations.iter().all(|d| d.init.is_some())
        {
            return;
        }
        if matches!(
            ctx.parent_kind(node),
            Some(
                AstKind::ForInStatement(_)
                    | AstKind::ForOfStatement(_)
                    | AstKind::ForStatement(_)
                    | AstKind::ExportNamedDeclaration(_)
            )
        ) {
            return;
        }
        let script_top_level =
            node.scope_id() == ctx.scoping().root_scope_id() && !util::has_private_top_level(ctx);
        if script_top_level {
            return;
        }
        let mut checked_ambient = false;
        for declarator in &decl.declarations {
            if declarator.init.is_some() {
                continue;
            }
            let BindingPattern::BindingIdentifier(id) = &declarator.id else {
                continue;
            };
            // `let nothing: undefined;` is undefined on purpose (e.g. to test type guards).
            if declarator
                .type_annotation
                .as_ref()
                .is_some_and(|t| is_nullish_type(&t.type_annotation))
            {
                continue;
            }
            let Some(symbol) = ctx.binding_symbol(id) else {
                continue;
            };
            if !ctx.scoping().symbol_redeclarations(symbol).is_empty() {
                continue;
            }
            let mut read = false;
            let mut written = false;
            for reference in ctx.references_of(symbol) {
                let flags = reference.flags();
                written |= flags.is_write();
                read |= flags.is_read();
            }
            if written || !read {
                continue;
            }
            if !checked_ambient {
                if ctx.in_ambient_context(node) {
                    return;
                }
                checked_ambient = true;
            }
            let name = id.name.as_str();
            ctx.emit(
                id.span,
                format!("Assign a value to '{name}' or remove it: it is read but never assigned, so it is always undefined"),
            );
        }
    }
}

/// `undefined`, `null`, `void`, or a union of them.
fn is_nullish_type(ty: &TSType<'_>) -> bool {
    match ty {
        TSType::TSUndefinedKeyword(_) | TSType::TSNullKeyword(_) | TSType::TSVoidKeyword(_) => true,
        TSType::TSUnionType(u) => u.types.iter().all(is_nullish_type),
        TSType::TSParenthesizedType(p) => is_nullish_type(&p.type_annotation),
        _ => false,
    }
}
