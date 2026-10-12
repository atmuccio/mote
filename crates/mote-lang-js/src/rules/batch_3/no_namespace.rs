//! `js/no-namespace`

use crate::prelude::*;

pub struct NoNamespace;

pub static NO_NAMESPACE: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["typescript", "modules", "conventions"],
    remediation_minutes: 15,
    description: r#"Flags TypeScript `namespace` (and legacy `module Name`) declarations.

**Why:** namespaces are TypeScript's pre-ES-module way of organising code. ES modules already give
every file its own scope and explicit imports/exports, which bundlers, tree shaking, and other tools
understand. Namespaces compile to extra runtime objects, cannot be tree-shaken, and are not
supported by tools that strip types without a full compiler (such as Babel or `isolatedModules`
builds in some configurations).

**Fix:** move the contents to their own module and `import` / `export` them.

Not flagged: `declare module 'name' { … }` (typing an external module), `declare global { … }`,
and `declare namespace` / anything inside an ambient declaration, which only describe code that
exists elsewhere.

```ts
export namespace Geometry {             // flagged
  export function area(r: number) { return Math.PI * r * r; }
}

// geometry.ts
export function area(r: number) { return Math.PI * r * r; } // compliant

declare module 'untyped-lib' {           // compliant
  export function run(): void;
}
```
"#,
    // Stylistic and noisy on real code: opt-in (`extends: all` or enable explicitly).
    default_enabled: false,
    ..RuleMeta::new(
        "js/no-namespace",
        "js",
        "ES modules should be used instead of namespaces",
    )
};

impl JsRule for NoNamespace {
    fn meta(&self) -> &'static RuleMeta {
        &NO_NAMESPACE
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::TSModuleDeclaration]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.is_typescript()
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::TSModuleDeclaration(decl) = node.kind() else {
            return;
        };
        let TSModuleDeclarationName::Identifier(id) = &decl.id else {
            return;
        };
        if decl.declare {
            return;
        }
        // `namespace A.B {}`: the inner `B` is part of the same declaration.
        if matches!(ctx.parent_kind(node), Some(AstKind::TSModuleDeclaration(_))) {
            return;
        }
        if ctx.in_ambient_context(node) {
            return;
        }
        let keyword = match decl.kind {
            TSModuleDeclarationKind::Namespace => "namespace",
            TSModuleDeclarationKind::Module => "module",
        };
        ctx.emit(
            id.span,
            format!(
                "Replace the {keyword} '{}' with an ES module",
                ctx.text(Span::new(id.span.start, qualified_end(decl)))
            ),
        );
    }
}

/// End offset of the full dotted name of `namespace A.B.C`.
fn qualified_end(decl: &TSModuleDeclaration<'_>) -> u32 {
    let mut end = decl.id.span().end;
    let mut body = decl.body.as_ref();
    while let Some(TSModuleDeclarationBody::TSModuleDeclaration(inner)) = body {
        end = inner.id.span().end;
        body = inner.body.as_ref();
    }
    end
}
