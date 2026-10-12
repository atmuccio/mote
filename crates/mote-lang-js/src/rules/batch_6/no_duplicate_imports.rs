//! `js/no-duplicate-imports`

use crate::prelude::*;

pub struct NoDuplicateImports;

pub static NO_DUPLICATE_IMPORTS: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["imports", "redundant"],
    remediation_minutes: 2,
    description: r#"Flags a second `import` declaration from the same module when the two could be written as one.

**Why:** the module is loaded once either way, but two declarations split the names taken from it
in two places. Readers have to look twice to see what the file uses from the module, edits add a
third line instead of extending the first, and merge conflicts on import lists get harder to
resolve.

**Fix:** move the names into the first declaration.

```ts
import { readFile } from 'fs';
import { writeFile } from 'fs';          // flagged

import { readFile, writeFile } from 'fs'; // compliant
```

Only declarations that fit in one are reported: a default import combines with named imports or
with a namespace import, but a namespace import cannot share a declaration with named imports, and
two default or two namespace imports cannot be merged. TypeScript `import type` declarations are
compared only with each other (a type-only default and type-only named imports cannot share one
either). Side-effect imports (`import './polyfill'`), imports with different attributes
(`with { type: 'json' }`), and imports inside different `declare module` blocks are left alone.
"#,
    ..RuleMeta::new(
        "js/no-duplicate-imports",
        "js",
        "Imports from the same module should be merged",
    )
};

#[derive(Clone, Copy)]
struct Shape {
    default: bool,
    namespace: bool,
    named: bool,
    type_only: bool,
}

impl Shape {
    /// Can the two declarations be written as one?
    fn merges_with(self, other: Shape) -> bool {
        let default = self.default && other.default;
        let namespace = self.namespace || other.namespace;
        let named = self.named || other.named;
        if default || (self.namespace && other.namespace) || (namespace && named) {
            return false;
        }
        // `import type D, { A } from 'x'` is not allowed.
        !(self.type_only && (self.default || other.default) && named)
    }
}

/// The block holding the import (the file or a `declare module`), the module specifier, whether
/// the import is type-only, and its attributes.
type ImportKey<'a> = (NodeId, &'a str, bool, &'a str);

/// The file's import declarations that bind names, in source order.
#[derive(Default)]
struct Imports {
    ids: Vec<NodeId>,
}

impl JsRule for NoDuplicateImports {
    fn meta(&self) -> &'static RuleMeta {
        &NO_DUPLICATE_IMPORTS
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::ImportDeclaration]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::ImportDeclaration(import) = node.kind() else {
            return;
        };
        let Some(specifiers) = &import.specifiers else {
            return;
        };
        if specifiers.is_empty() || import.phase.is_some() {
            return;
        }
        ctx.state::<Imports>().ids.push(node.id());
    }

    fn run_once<'a>(&self, ctx: &mut Ctx<'a>) {
        let ids = std::mem::take(&mut ctx.state::<Imports>().ids);
        if ids.len() < 2 {
            return;
        }
        // Most files import each module once: look for a repeated specifier before grouping.
        let mut sources: Vec<&str> = ids
            .iter()
            .filter_map(|&id| match ctx.node(id).kind() {
                AstKind::ImportDeclaration(import) => Some(import.source.value.as_str()),
                _ => None,
            })
            .collect();
        sources.sort_unstable();
        if !sources.windows(2).any(|pair| pair[0] == pair[1]) {
            return;
        }
        // Earlier declarations per key (keys borrow the file's text).
        let mut seen: FxHashMap<ImportKey<'a>, Vec<(Span, Shape)>> = FxHashMap::default();
        for id in ids {
            let node = ctx.node(id);
            let AstKind::ImportDeclaration(import) = node.kind() else {
                continue;
            };
            let Some(specifiers) = &import.specifiers else {
                continue;
            };
            let type_only = import.import_kind.is_type();
            let mut shape = Shape {
                default: false,
                namespace: false,
                named: false,
                type_only,
            };
            for specifier in specifiers {
                match specifier {
                    ImportDeclarationSpecifier::ImportDefaultSpecifier(_) => shape.default = true,
                    ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => {
                        shape.namespace = true;
                    }
                    ImportDeclarationSpecifier::ImportSpecifier(_) => shape.named = true,
                }
            }
            let container = ctx
                .ancestors(node)
                .find(|n| {
                    matches!(
                        n.kind(),
                        AstKind::TSModuleDeclaration(_) | AstKind::Program(_)
                    )
                })
                .map_or(NodeId::ROOT, AstNode::id);
            let attributes = import.with_clause.as_ref().map_or("", |w| ctx.text(w.span));
            let source = import.source.value.as_str();
            let key = (container, source, type_only, attributes);
            let earlier = seen.entry(key).or_default();
            let first = earlier
                .iter()
                .find(|(_, other)| shape.merges_with(*other))
                .map(|&(span, _)| span);
            earlier.push((import.span, shape));
            let Some(first) = first else {
                continue;
            };
            let span = Span::new(import.span.start, import.span.start + 6);
            let first_span = ctx
                .text(first)
                .find('\n')
                .map_or(first, |i| Span::new(first.start, first.start + i as u32));
            ctx.emit(
                span,
                format!("Merge this import with the earlier import from '{source}'"),
            )
            .secondary(first_span, "First import from the module");
        }
    }
}
