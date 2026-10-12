//! `js/prefer-const`

use super::util;
use crate::prelude::*;

pub struct PreferConst;

pub static PREFER_CONST: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["convention"],
    remediation_minutes: 1,
    params: &[ParamSpec {
        name: "destructuring",
        default: ParamDefault::Str("all"),
        description: "`all`: report a declaration only when none of its variables is reassigned. \
                      `any`: also report the never-reassigned variables of a declaration whose \
                      other variables are reassigned (the declaration must then be split).",
    }],
    description: r#"Flags `let` declarations whose variables are never assigned again after being
initialized, and `let x;` declarations whose only assignment follows in the same block.

**Why:** `const` tells the reader up front that the binding keeps its value for its whole scope,
so they don't have to scan the rest of the function for reassignments. It also lets tools catch
accidental reassignment.

**Fix:** declare the variable with `const`. A declaration is only reported when *every* variable
it declares is never reassigned (so `let i = 0, n = a.length` in a loop where `i` changes is left
alone), and every declarator has an initializer (or it is the head of a `for…of` / `for…in` loop).
Mutating the object a variable points to (`list.push(x)`, `obj.a = 1`) is not a reassignment.

A variable declared without a value (`let user;`) is reported at its assignment when that
assignment is the only one, is a plain statement in the same block as the declaration, and nothing
uses the variable before it: declare it there with `const` instead.

```js
let total = price * qty;       // flagged: never reassigned
for (let item of items) {}     // flagged

let user;
user = await load(id);         // flagged: declare it here with `const`

const total = price * qty;     // compliant
let count = 0;                 // compliant: reassigned below
count++;
```

With the `destructuring` parameter set to `any`, a declaration whose variables are only partly
reassigned (`let { a, b } = obj; b = 1;`) is reported too, naming the variables that could be
`const` (the declaration has to be split). Top-level `let` in non-module scripts is not reported:
another script can assign it.
"#,
    ..RuleMeta::new(
        "js/prefer-const",
        "js",
        "Variables that are never reassigned should be declared with 'const'",
    )
};

impl JsRule for PreferConst {
    fn meta(&self) -> &'static RuleMeta {
        &PREFER_CONST
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::VariableDeclaration]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        // Direct `eval` can assign any variable in scope by name.
        !util::uses_global(ctx, "eval")
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::VariableDeclaration(decl) = node.kind() else {
            return;
        };
        if decl.kind != VariableDeclarationKind::Let || decl.declare {
            return;
        }
        let parent = ctx.parent_kind(node);
        let loop_head = matches!(
            parent,
            Some(AstKind::ForInStatement(_) | AstKind::ForOfStatement(_))
        );
        if !loop_head && decl.declarations.iter().any(|d| d.init.is_none()) {
            check_assigned_once(ctx, node, decl);
            return;
        }
        let mut names: Vec<&'a str> = Vec::new();
        let mut reassigned = false;
        for declarator in &decl.declarations {
            helpers::for_each_binding(&declarator.id, &mut |id| {
                let fixed = id.symbol_id.get().is_some_and(|symbol| {
                    ctx.scoping().symbol_redeclarations(symbol).is_empty()
                        && !ctx.references_of(symbol).any(|r| r.flags().is_write())
                });
                if fixed {
                    names.push(id.name.as_str());
                } else {
                    reassigned = true;
                }
            });
        }
        if names.is_empty() {
            return;
        }
        // `for (let i = 0, n = len; …; i++)`: the head holds a single declaration.
        if reassigned
            && (matches!(parent, Some(AstKind::ForStatement(_)))
                || ctx.param_str("destructuring") != "any")
        {
            return;
        }
        if !top_level_can_change(ctx, node) {
            return;
        }
        let keyword = Span::new(decl.span.start, decl.span.start + 3);
        if ctx.text(keyword) != "let" {
            return;
        }
        let listed = || {
            let shown: Vec<String> = names.iter().take(4).map(|n| format!("'{n}'")).collect();
            let more = if names.len() > 4 { ", …" } else { "" };
            format!("{}{more}", shown.join(", "))
        };
        let message = match (names.as_slice(), reassigned) {
            ([one], false) => format!("Use 'const' instead of 'let': '{one}' is never reassigned"),
            (_, false) => format!(
                "Use 'const' instead of 'let': {} are never reassigned",
                listed()
            ),
            ([one], true) => format!(
                "Split this declaration and use 'const' for '{one}': it is never reassigned"
            ),
            (_, true) => format!(
                "Split this declaration and use 'const' for {}: they are never reassigned",
                listed()
            ),
        };
        ctx.emit(keyword, message);
    }
}

/// `false` for top-level declarations of a classic script (shared with every other script) and
/// for declarations in ambient contexts.
fn top_level_can_change(ctx: &Ctx<'_>, node: &AstNode<'_>) -> bool {
    if node.scope_id() == ctx.scoping().root_scope_id() && !util::has_private_top_level(ctx) {
        return false;
    }
    !ctx.in_ambient_context(node)
}

/// `let x; … x = value;`: a single declarator without initializer whose only write is a plain
/// assignment statement in the same statement list, with no use of the variable before it. The
/// declaration can move to the assignment as `const x = value;`.
fn check_assigned_once<'a>(ctx: &mut Ctx<'a>, node: &AstNode<'a>, decl: &VariableDeclaration<'a>) {
    let [declarator] = decl.declarations.as_slice() else {
        return;
    };
    let BindingPattern::BindingIdentifier(id) = &declarator.id else {
        return;
    };
    if declarator.init.is_some() {
        return;
    }
    let Some(symbol) = ctx.binding_symbol(id) else {
        return;
    };
    if !ctx.scoping().symbol_redeclarations(symbol).is_empty() {
        return;
    }
    let mut write = None;
    for reference in ctx.references_of(symbol) {
        if reference.flags().is_write() {
            if write.is_some() {
                return;
            }
            write = Some(reference.node_id());
        }
    }
    let Some(write) = write else {
        return;
    };
    // The write must be `x = value;` as a statement of the list that holds the declaration.
    let Some(assign_node) = ctx.parent(write) else {
        return;
    };
    let AstKind::AssignmentExpression(assign) = assign_node.kind() else {
        return;
    };
    if assign.operator != AssignmentOperator::Assign
        || !matches!(&assign.left, AssignmentTarget::AssignmentTargetIdentifier(target) if target.span == ctx.node(write).span())
    {
        return;
    }
    let Some(statement) = ctx.parent(assign_node) else {
        return;
    };
    if !matches!(statement.kind(), AstKind::ExpressionStatement(_)) {
        return;
    }
    let (Some(list), Some(own_list)) = (ctx.parent(statement), ctx.parent(node)) else {
        return;
    };
    if list.id() != own_list.id()
        || !matches!(
            list.kind(),
            AstKind::BlockStatement(_)
                | AstKind::FunctionBody(_)
                | AstKind::Program(_)
                | AstKind::SwitchCase(_)
                | AstKind::StaticBlock(_)
                | AstKind::TSModuleBlock(_)
        )
    {
        return;
    }
    // Nothing may use the variable before (or while computing) its value, and no hoisted function
    // declaration may read it: it could run before the `const` is initialized.
    let assigned_at = statement.span().end;
    for reference in ctx.references_of(symbol) {
        if reference.node_id() == write {
            continue;
        }
        if ctx.node(reference.node_id()).span().start < assigned_at {
            return;
        }
        let in_hoisted_function = ctx
            .ancestors(reference.node_id())
            .take_while(|a| a.id() != own_list.id())
            .any(|a| matches!(a.kind(), AstKind::Function(f) if f.is_declaration()));
        if in_hoisted_function {
            return;
        }
    }
    if !top_level_can_change(ctx, node) {
        return;
    }
    let target = ctx.node(write).span();
    ctx.emit(
        target,
        format!(
            "Declare '{}' with 'const' here: this is its only assignment",
            id.name
        ),
    )
    .secondary(
        Span::new(decl.span.start, id.span.end),
        "Declared without a value here",
    );
}
