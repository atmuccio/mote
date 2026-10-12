//! `js/block-scoped-var`

use crate::prelude::*;

pub struct BlockScopedVar;

pub static BLOCK_SCOPED_VAR: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["confusing", "es2015"],
    remediation_minutes: 5,
    description: r#"Flags a `var` declared inside a block (an `if`, a loop, a `switch`, a plain `{ }`) that is used
outside that block.

**Why:** `var` ignores blocks: the variable belongs to the whole function, and exists (as
`undefined`) before and after the block. Code that reads it outside the block depends on hoisting
— it sees `undefined` when the block did not run, or the value from the last loop iteration —
which readers rarely expect and which breaks as soon as the declaration is changed to `let` or
`const`.

**Fix:** declare the variable before the block, at the level where it is used, preferably with
`let` or `const`.

```js
if (user) {
  var name = user.name;
}
greet(name);                 // flagged: `name` is undefined when there is no user

let name;                    // compliant
if (user) {
  name = user.name;
}
greet(name);
```

A variable declared with `var` in several blocks counts as declared in each of them. Off by default:
this is a style rule for code that still uses `var`.
"#,
    ..RuleMeta::new(
        "js/block-scoped-var",
        "js",
        "Variables declared with var should not be used outside the block that declares them",
    )
};

impl JsRule for BlockScopedVar {
    fn meta(&self) -> &'static RuleMeta {
        &BLOCK_SCOPED_VAR
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::VariableDeclaration]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::VariableDeclaration(decl) = node.kind() else {
            return;
        };
        if decl.kind != VariableDeclarationKind::Var || decl.declare {
            return;
        }
        // Only the first declarator's statement is examined; the symbol-level check below looks at
        // every declaration of each name.
        for declarator in &decl.declarations {
            helpers::for_each_binding(&declarator.id, &mut |id| {
                let Some(symbol) = ctx.binding_symbol(id) else {
                    return;
                };
                // Examine each symbol once, from its first declaration.
                if ctx.symbol_span(symbol) != id.span {
                    return;
                }
                check_symbol(ctx, symbol);
            });
        }
    }
}

fn check_symbol(ctx: &mut Ctx<'_>, symbol: SymbolId) {
    let scoping = ctx.scoping();
    let mut declarations = vec![scoping.symbol_declaration(symbol)];
    declarations.extend(
        scoping
            .symbol_redeclarations(symbol)
            .iter()
            .map(|r| r.declaration),
    );
    let mut blocks = Vec::new();
    for declaration in declarations {
        let node = ctx.node(declaration);
        if !matches!(node.kind(), AstKind::VariableDeclarator(d) if d.kind == VariableDeclarationKind::Var)
        {
            return;
        }
        match declaring_block(ctx, node) {
            Some(span) => blocks.push(span),
            // Declared at function level somewhere: the variable is meant for the whole function.
            None => return,
        }
    }
    let name = ctx.symbol_name(symbol);
    let mut outside = Vec::new();
    for reference in ctx.references_of(symbol) {
        let span = ctx.node(reference.node_id()).span();
        if !blocks
            .iter()
            .any(|b| b.start <= span.start && span.end <= b.end)
        {
            outside.push(span);
        }
    }
    for span in outside {
        ctx.emit(
            span,
            format!("'{name}' is used outside the block that declares it: declare it with 'let' before the block"),
        );
    }
}

/// The block (statement block, loop, or `switch`) that holds the declaration, or `None` when it is
/// declared at the level of its function or script.
fn declaring_block(ctx: &Ctx<'_>, declarator: &AstNode<'_>) -> Option<Span> {
    for ancestor in ctx.ancestors(declarator) {
        match ancestor.kind() {
            AstKind::BlockStatement(_)
            | AstKind::ForStatement(_)
            | AstKind::ForInStatement(_)
            | AstKind::ForOfStatement(_)
            | AstKind::SwitchStatement(_)
            | AstKind::StaticBlock(_) => return Some(ancestor.span()),
            AstKind::FunctionBody(_) | AstKind::Program(_) | AstKind::TSModuleBlock(_) => {
                return None
            }
            // `if (x) var y;`, `label: var z;`: no block of their own.
            AstKind::IfStatement(_) | AstKind::LabeledStatement(_) | AstKind::WhileStatement(_) => {
                return None
            }
            _ => {}
        }
    }
    None
}
