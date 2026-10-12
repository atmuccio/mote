//! `js/no-useless-empty-export`

use crate::prelude::*;

pub struct NoUselessEmptyExport;

pub static NO_USELESS_EMPTY_EXPORT: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["redundant", "modules"],
    remediation_minutes: 1,
    description: r#"Flags `export {}` in a file that already imports or exports something else.

**Why:** an empty `export {}` has exactly one purpose: turning a file without any `import` or
`export` into an ES module (so its declarations stay private to the file). As soon as the file has
another `import` or `export`, it is a module anyway and the empty export is dead code that makes
readers wonder what it is for.

**Fix:** delete the `export {}`.

Type-only imports and exports (`import type`, `export type`) do not count as "something else": they
are removed from the compiled output, so the empty export may still be what keeps the emitted file a
module.

```ts
import { run } from './run';
export {};                     // flagged

// globals.ts — nothing else is imported or exported
declare global { interface Window { app: App } }
export {};                     // compliant: makes this file a module
```
"#,
    ..RuleMeta::new(
        "js/no-useless-empty-export",
        "js",
        "Empty 'export {}' should be removed when the file is already a module",
    )
};

impl JsRule for NoUselessEmptyExport {
    fn meta(&self) -> &'static RuleMeta {
        &NO_USELESS_EMPTY_EXPORT
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::Program]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.file_contains(AstType::ExportNamedDeclaration)
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::Program(program) = node.kind() else {
            return;
        };
        let mut empty_exports: Vec<Span> = Vec::new();
        let mut other_module_syntax = false;
        for stmt in &program.body {
            match stmt {
                Statement::ExportNamedDeclaration(e)
                    if e.declaration.is_none() && e.specifiers.is_empty() && e.source.is_none() =>
                {
                    empty_exports.push(e.span);
                }
                Statement::ExportNamedDeclaration(e) => {
                    let type_only = e.export_kind.is_type()
                        || e.declaration.as_ref().is_some_and(|d| {
                            matches!(
                                d,
                                Declaration::TSTypeAliasDeclaration(_)
                                    | Declaration::TSInterfaceDeclaration(_)
                            ) || d.declare()
                        })
                        || (e.source.is_none()
                            && !e.specifiers.is_empty()
                            && e.specifiers.iter().all(|s| s.export_kind.is_type()));
                    other_module_syntax |= !type_only;
                }
                Statement::ImportDeclaration(i) => {
                    let type_only = i.import_kind.is_type()
                        || i.specifiers.as_ref().is_some_and(|specs| {
                            !specs.is_empty()
                                && specs.iter().all(|s| {
                                    matches!(s, ImportDeclarationSpecifier::ImportSpecifier(s) if s.import_kind.is_type())
                                })
                        });
                    other_module_syntax |= !type_only;
                }
                Statement::ExportDefaultDeclaration(_)
                | Statement::ExportAllDeclaration(_)
                | Statement::TSExportAssignment(_) => other_module_syntax = true,
                Statement::TSImportEqualsDeclaration(i) => {
                    other_module_syntax |= !i.import_kind.is_type()
                        && matches!(
                            i.module_reference,
                            TSModuleReference::ExternalModuleReference(_)
                        );
                }
                _ => {}
            }
        }
        if !other_module_syntax {
            return;
        }
        for span in empty_exports {
            ctx.emit(
                span,
                "Remove this empty 'export {}': the file already has imports or exports",
            );
        }
    }
}
