//! `js/mutable-export` (off by default; part of the `conventions` set)

use crate::prelude::*;

pub struct MutableExport;

pub static MUTABLE_EXPORT: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["conventions", "modules"],
    remediation_minutes: 5,
    description: r#"Flags exported bindings that can be reassigned: `export let` and `export var`, and
`export { name }` of a module-level `let` or `var` that the module assigns again.

**Why:** an ES module export is a live binding. When the exporting module reassigns it, every
importer sees the new value at some later point, which is shared mutable global state in
disguise: the value an importer reads depends on timing and import order, and importers cannot
assign it themselves. Most of the time the binding never changes and should simply be `const`;
when it does change, an explicit getter or setter function makes the change visible.

**Fix:** export a `const`, or keep the mutable variable private and export functions that read
or change it.

```js
export let currentUser = null;            // flagged
export function setUser(user) { currentUser = user; }

let currentUser = null;                   // compliant
export function getUser() { return currentUser; }
export function setUser(user) { currentUser = user; }
```

Not reported: `export { name }` of a `let` that is never assigned after its declaration (it is
constant in practice; `js/prefer-const` reports the `let`), `declare` statements and code inside
`declare module` / `declare global`.

Off by default: part of the opt-in `conventions` set.
"#,
    ..RuleMeta::new(
        "js/mutable-export",
        "js",
        "Exported bindings should not be mutable",
    )
};

impl JsRule for MutableExport {
    fn meta(&self) -> &'static RuleMeta {
        &MUTABLE_EXPORT
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::ExportNamedDeclaration]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::ExportNamedDeclaration(export) = node.kind() else {
            return;
        };
        if export.export_kind.is_type() {
            return;
        }
        if !matches!(ctx.parent_kind(node), Some(AstKind::Program(_))) {
            return; // namespaces and `declare module` blocks
        }
        if let Some(Declaration::VariableDeclaration(decl)) = &export.declaration {
            if decl.declare
                || !matches!(
                    decl.kind,
                    VariableDeclarationKind::Let | VariableDeclarationKind::Var
                )
            {
                return;
            }
            let keyword = decl.kind.as_str();
            let mut names = Vec::new();
            for declarator in &decl.declarations {
                helpers::for_each_binding(&declarator.id, &mut |id| names.push(id.name.as_str()));
            }
            let span = Span::new(decl.span.start, decl.span.start + keyword.len() as u32);
            ctx.emit(
                span,
                format!(
                    "Export a 'const' or accessor functions instead of the mutable '{keyword}' binding '{}'",
                    names.join("', '")
                ),
            );
            return;
        }
        if export.source.is_some() {
            return;
        }
        for specifier in &export.specifiers {
            if specifier.export_kind.is_type() {
                continue;
            }
            let ModuleExportName::IdentifierReference(local) = &specifier.local else {
                continue;
            };
            let Some(symbol) = ctx.reference_symbol(local) else {
                continue;
            };
            let decl = ctx.symbol_declaration(symbol);
            let AstKind::VariableDeclarator(declarator) = decl.kind() else {
                continue;
            };
            if !matches!(
                declarator.kind,
                VariableDeclarationKind::Let | VariableDeclarationKind::Var
            ) {
                continue;
            }
            let module_level = ctx
                .parent(decl)
                .and_then(|declaration| ctx.parent_kind(declaration))
                .is_some_and(|kind| matches!(kind, AstKind::Program(_)));
            if !module_level {
                continue;
            }
            if !ctx.references_of(symbol).any(|r| r.is_write()) {
                continue;
            }
            let keyword = declarator.kind.as_str();
            ctx.emit(
                specifier.span,
                format!(
                    "Export a 'const' or accessor functions instead of the reassigned '{keyword}' binding '{}'",
                    local.name
                ),
            );
        }
    }
}
