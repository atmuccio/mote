//! `js/no-require-imports`

use crate::prelude::*;

pub struct NoRequireImports;

pub static NO_REQUIRE_IMPORTS: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["typescript", "modules"],
    remediation_minutes: 5,
    description: r#"Flags CommonJS `require()` calls and `import x = require('x')` in TypeScript
files.

**Why:** in TypeScript, `require()` returns `any` unless the result is annotated by hand, so
everything taken from the module escapes type checking. Mixing `require` with ES `import`s also
makes the module format of a file unclear and defeats tools that rely on static imports (tree
shaking, bundlers, `isolatedModules`).

**Fix:** use an ES `import` statement, or a dynamic `import()` for modules loaded on demand.

Not flagged: a local function or parameter named `require` (AMD loaders, `createRequire`), and
`.cts` files, which are CommonJS by definition.

```ts
const fs = require('fs');              // flagged
import path = require('path');         // flagged

import * as fs from 'fs';              // compliant
const lazy = await import('./heavy');  // compliant
```
"#,
    ..RuleMeta::new(
        "js/no-require-imports",
        "js",
        "ES imports should be used instead of require()",
    )
};

impl JsRule for NoRequireImports {
    fn meta(&self) -> &'static RuleMeta {
        &NO_REQUIRE_IMPORTS
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CallExpression, AstType::TSImportEqualsDeclaration]
    }

    fn callee_names(&self) -> &'static [&'static str] {
        &["require"]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.is_typescript() && !ctx.source_type().is_commonjs() && !ctx.path().ends_with(".cts")
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        match node.kind() {
            AstKind::CallExpression(call) => {
                let Expression::Identifier(callee) = &call.callee else {
                    return;
                };
                if callee.name != "require" || !ctx.is_global_reference(callee) {
                    return;
                }
                if ctx.in_ambient_context(node) {
                    return;
                }
                ctx.emit(
                    call.span,
                    "Replace this 'require()' call with an ES 'import'",
                );
            }
            AstKind::TSImportEqualsDeclaration(decl) => {
                if decl.import_kind.is_type() {
                    return;
                }
                if !matches!(
                    decl.module_reference,
                    TSModuleReference::ExternalModuleReference(_)
                ) {
                    return;
                }
                if ctx.in_ambient_context(node) {
                    return;
                }
                ctx.emit(
                    decl.module_reference.span(),
                    format!(
                        "Replace 'import {} = require(...)' with an ES 'import'",
                        decl.id.name
                    ),
                );
            }
            _ => {}
        }
    }
}
