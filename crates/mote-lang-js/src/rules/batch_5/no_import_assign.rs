//! `js/no-import-assign`

use super::util;
use crate::prelude::*;

pub struct NoImportAssign;

pub static NO_IMPORT_ASSIGN: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["modules", "suspicious"],
    remediation_minutes: 5,
    description: r#"Flags writes to an imported binding (`import { count } from './a'; count = 1;`) and to a
property of a namespace import (`import * as utils from './u'; utils.helper = …;`).

**Why:** ES module imports are live, read-only views of the exporting module's bindings, and a
namespace object is sealed. Assigning to either throws a `TypeError` in a real ES module runtime.
Code that seems to work today usually does so only because a bundler or test runner turned the
module into CommonJS — it breaks as soon as it runs as native ESM.

**Fix:** copy the value into a local variable and change that, or have the exporting module
provide a function that updates its own state. To replace a module's functions in tests, use the
test framework's module mocking instead of assigning to the namespace object.

```js
import { retries } from './config.js';
retries = 5;                     // flagged: throws TypeError

import * as fs from 'node:fs';
fs.readFile = fakeReadFile;      // flagged: namespace objects are read-only

import { retries } from './config.js';
let attempts = retries;          // compliant
attempts = 5;
```

Writes to properties of a *named* or default import (`config.debug = true`) are allowed: they
change the imported object, not the binding. Namespace property writes in test files are not
reported, because many test setups compile modules to CommonJS on purpose to allow monkey-patching.
"#,
    ..RuleMeta::new(
        "js/no-import-assign",
        "js",
        "Imported bindings should not be reassigned",
    )
};

impl JsRule for NoImportAssign {
    fn meta(&self) -> &'static RuleMeta {
        &NO_IMPORT_ASSIGN
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.file_contains(AstType::ImportDeclaration)
    }

    fn run_once<'a>(&self, ctx: &mut Ctx<'a>) {
        let mut reports = Vec::new();
        for (reference, symbol) in util::written_references(ctx, !ctx.is_test()) {
            let flags = ctx.symbol_flags(symbol);
            if !flags.contains(SymbolFlags::Import) || flags.contains(SymbolFlags::TypeImport) {
                continue;
            }
            let declaration = ctx.symbol_declaration(symbol).kind();
            if !matches!(
                declaration,
                AstKind::ImportSpecifier(_)
                    | AstKind::ImportDefaultSpecifier(_)
                    | AstKind::ImportNamespaceSpecifier(_)
            ) {
                continue;
            }
            if reference.is_write() {
                reports.push((ctx.node(reference.node_id()).span(), symbol, false));
            } else if matches!(declaration, AstKind::ImportNamespaceSpecifier(_)) {
                if let Some(span) = direct_member_write(ctx, reference.node_id()) {
                    reports.push((span, symbol, true));
                }
            }
        }
        for (span, symbol, member) in reports {
            let name = ctx.symbol_name(symbol);
            let message = if member {
                format!("Do not modify the namespace import '{name}': its properties are read-only and this throws a TypeError in an ES module")
            } else {
                format!("Do not reassign the import '{name}': imported bindings are read-only and this throws a TypeError")
            };
            ctx.emit(span, message);
        }
    }
}

/// The span of the member expression when the reference is the object of a member expression that
/// is itself written (`ns.x = 1`, `ns.x++`, `delete ns.x`), not a deeper one (`ns.x.y = 1`).
fn direct_member_write(ctx: &Ctx<'_>, reference: NodeId) -> Option<Span> {
    let member = ctx.parent(reference)?;
    let member_span = match member.kind() {
        AstKind::StaticMemberExpression(m) => m.span,
        AstKind::ComputedMemberExpression(m) => m.span,
        _ => return None,
    };
    let written = match ctx.parent_kind(member)? {
        AstKind::AssignmentExpression(a) => a.left.span() == member_span,
        AstKind::UpdateExpression(_) => true,
        AstKind::UnaryExpression(u) => u.operator == UnaryOperator::Delete,
        AstKind::ForInStatement(f) => f.left.span() == member_span,
        AstKind::ForOfStatement(f) => f.left.span() == member_span,
        _ => false,
    };
    written.then_some(member_span)
}
