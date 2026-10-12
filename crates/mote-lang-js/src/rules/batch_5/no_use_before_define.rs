//! `js/no-use-before-define`

use super::util;
use crate::prelude::*;

pub struct NoUseBeforeDefine;

pub static NO_USE_BEFORE_DEFINE: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["suspicious", "scope"],
    remediation_minutes: 5,
    description: r#"Flags a `let`, `const`, or `class` binding that is used before its declaration has run, in the
same function: `print(total); const total = 1;` or `const size = size + 1;`.

**Why:** unlike `var` and function declarations, these bindings exist from the start of their scope
but stay uninitialized (the "temporal dead zone") until the declaration executes. Any read or write
before that — even `typeof total` — throws a `ReferenceError`. It typically happens when statements
are reordered, or when a helper is moved below the code that uses it.

**Fix:** move the declaration above its first use, or move the use below it.

```js
render(config);                 // flagged: ReferenceError
const config = loadConfig();

const config = loadConfig();    // compliant
render(config);

function show() {
  return formatter.format(1);   // compliant: runs later, when show() is called
}
const formatter = new Intl.NumberFormat();
```

Only uses that certainly run first are reported: a use inside a function, arrow function, or class
body is skipped, because it runs only when called — possibly after the declaration. Type-only uses
in TypeScript and `export { name }` lists are not uses. `var` and function declarations are hoisted
and never reported.
"#,
    ..RuleMeta::new(
        "js/no-use-before-define",
        "js",
        "Variables should not be used before their declaration",
    )
};

impl JsRule for NoUseBeforeDefine {
    fn meta(&self) -> &'static RuleMeta {
        &NO_USE_BEFORE_DEFINE
    }

    fn run_once<'a>(&self, ctx: &mut Ctx<'a>) {
        let scoping = ctx.scoping();
        let mut reports = Vec::new();
        for symbol in scoping.symbol_ids() {
            let flags = scoping.symbol_flags(symbol);
            if !flags.intersects(SymbolFlags::BlockScopedVariable | SymbolFlags::Class)
                || flags.intersects(
                    SymbolFlags::Ambient
                        | SymbolFlags::Import
                        | SymbolFlags::Function
                        | SymbolFlags::FunctionScopedVariable,
                )
            {
                continue;
            }
            let declaration = ctx.symbol_declaration(symbol);
            // The range in which a use runs before the binding is initialized: everything before
            // the name, plus the initializer of a variable (`const x = x + 1`).
            let binding = ctx.symbol_span(symbol);
            let (initializer, owner_of) = match declaration.kind() {
                AstKind::VariableDeclarator(d) => {
                    if !matches!(
                        d.kind,
                        VariableDeclarationKind::Let
                            | VariableDeclarationKind::Const
                            | VariableDeclarationKind::Using
                            | VariableDeclarationKind::AwaitUsing
                    ) {
                        continue;
                    }
                    (d.init.as_ref().map(GetSpan::span), declaration)
                }
                AstKind::Class(c) => {
                    if c.declare || !c.is_declaration() {
                        continue;
                    }
                    let Some(parent) = ctx.parent(declaration) else {
                        continue;
                    };
                    (None, parent)
                }
                _ => continue,
            };
            let mut declaration_owner = None;
            let mut ambient = None;
            for reference in ctx.references_of(symbol) {
                if !reference.is_value() || reference.flags().is_value_as_type() {
                    continue;
                }
                let node = ctx.node(reference.node_id());
                let at = node.span().start;
                let early =
                    at < binding.start || initializer.is_some_and(|i| i.start <= at && at < i.end);
                if !early {
                    continue;
                }
                if ctx.ancestors(node).take(3).any(|a| {
                    matches!(
                        a.kind(),
                        AstKind::ExportSpecifier(_)
                            | AstKind::TSTypeQuery(_)
                            | AstKind::TSExportAssignment(_)
                    )
                }) {
                    continue;
                }
                if *ambient.get_or_insert_with(|| ctx.in_ambient_context(declaration)) {
                    break;
                }
                let owner =
                    *declaration_owner.get_or_insert_with(|| util::deferred_owner(ctx, owner_of));
                if util::deferred_owner(ctx, node) != owner {
                    continue;
                }
                reports.push((node.span(), symbol));
            }
        }
        for (span, symbol) in reports {
            let name = ctx.symbol_name(symbol);
            ctx.emit(
                span,
                format!("Move the declaration of '{name}' before this use: it runs first and throws a ReferenceError"),
            );
        }
    }
}
