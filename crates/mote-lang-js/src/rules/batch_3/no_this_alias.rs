//! `js/no-this-alias`

use crate::prelude::*;

pub struct NoThisAlias;

pub static NO_THIS_ALIAS: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["style"],
    remediation_minutes: 5,
    description: r#"Flags `this` being stored in a variable (`const self = this`, `that = this`).

**Why:** copying `this` into a variable was the pre-ES2015 way to reach the outer `this` from a
nested `function` callback. Arrow functions keep the surrounding `this`, so the alias is no longer
needed; it adds a second name for the same object and makes readers check which one each callback
uses.

**Fix:** turn the nested callbacks into arrow functions and use `this` directly. Destructuring
(`const { items } = this`) is fine and not flagged.

Not flagged either, because an arrow function cannot replace the alias there: variables used inside
a class body (`new class extends Base { run() { that.go(); } }`), an object-literal method, getter,
or setter, a generator function, or a `function` that also needs its own `this` or `arguments`
(`el.onclick = function () { that.log(this.id); }`). Nor are variables read by code written before
they receive `this` (a callback created before `super()` returns), or variables assigned again
later: such a variable is a cursor that merely starts at `this`
(`let node = this; while (node) { node = node.parent; }`).

```js
const self = this;                       // flagged
button.onclick = function () { self.save(); };

button.onclick = () => this.save();      // compliant
const { items, total } = this;           // compliant: destructuring
```
"#,
    ..RuleMeta::new(
        "js/no-this-alias",
        "js",
        "'this' should not be assigned to a variable",
    )
};

impl JsRule for NoThisAlias {
    fn meta(&self) -> &'static RuleMeta {
        &NO_THIS_ALIAS
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::VariableDeclarator, AstType::AssignmentExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let (id_span, name, symbol, own_writes) = match node.kind() {
            AstKind::VariableDeclarator(decl) => {
                let Some(init) = &decl.init else {
                    return;
                };
                if !matches!(helpers::unwrap_expr(init), Expression::ThisExpression(_)) {
                    return;
                }
                let BindingPattern::BindingIdentifier(id) = &decl.id else {
                    return; // destructuring is fine
                };
                (id.span, id.name.as_str(), ctx.binding_symbol(id), 0)
            }
            AstKind::AssignmentExpression(assign) => {
                if assign.operator != AssignmentOperator::Assign
                    || !matches!(
                        helpers::unwrap_expr(&assign.right),
                        Expression::ThisExpression(_)
                    )
                {
                    return;
                }
                let AssignmentTarget::AssignmentTargetIdentifier(id) = &assign.left else {
                    return;
                };
                (id.span, id.name.as_str(), ctx.reference_symbol(id), 1)
            }
            _ => return,
        };
        let Some(symbol) = symbol else {
            return;
        };
        if alias_is_needed(ctx, symbol, own_writes, id_span.start) {
            return;
        }
        ctx.emit(
            id_span,
            format!("Use 'this' directly (with arrow functions) instead of the alias '{name}'"),
        );
    }
}

/// The alias is not the "reach `this` from a callback" idiom, or cannot be replaced by arrow
/// functions:
///
/// * the variable is assigned again later — a cursor that merely starts at `this`
///   (`let node = this; while (node) node = node.parent;`);
/// * it is used inside a class body, an object-literal method/getter/setter, a generator, or a
///   `function` that relies on its own `this` or `arguments` — none of these can become an arrow
///   function that keeps the outer `this`.
fn alias_is_needed(ctx: &Ctx<'_>, symbol: SymbolId, own_writes: usize, alias_at: u32) -> bool {
    if ctx.references_of(symbol).filter(|r| r.is_write()).count() > own_writes {
        return true;
    }
    // Code written before the alias receives `this` (a closure created before `super()` returns,
    // reading the variable later) needs a variable that is filled in afterwards.
    if ctx
        .references_of(symbol)
        .any(|r| !r.is_write() && ctx.node(r.node_id()).span().start < alias_at)
    {
        return true;
    }
    let declaration = ctx.symbol_declaration(symbol);
    let scope_owner = ctx.enclosing_function(declaration).map(AstNode::id);
    let mut checked: Vec<NodeId> = Vec::new();
    ctx.references_of(symbol).any(|reference| {
        for ancestor in ctx.ancestors(reference.node_id()) {
            if Some(ancestor.id()) == scope_owner {
                return false;
            }
            match ancestor.kind() {
                AstKind::Class(_) => return true,
                AstKind::ObjectProperty(p) if p.method || p.kind != PropertyKind::Init => {
                    return true;
                }
                AstKind::Function(f) => {
                    if f.generator || f.this_param.is_some() {
                        return true;
                    }
                    if !checked.contains(&ancestor.id()) {
                        checked.push(ancestor.id());
                        if uses_own_this_or_arguments(ctx, ancestor) {
                            return true;
                        }
                    }
                }
                _ => {}
            }
        }
        false
    })
}

/// Does a `function` use its own `this` or `arguments` (looking through arrow functions, which
/// share them, but not into nested functions or classes)? Nodes are stored in source pre-order,
/// so the function's subtree is the run of nodes after it that start before its end.
fn uses_own_this_or_arguments(ctx: &Ctx<'_>, function: &AstNode<'_>) -> bool {
    let outer = function.span();
    let mut skip_until = 0u32;
    for node in ctx.nodes().iter().skip(function.id().index() + 1) {
        let span = node.span();
        if span.start >= outer.end {
            break;
        }
        if span.start < skip_until {
            continue;
        }
        match node.kind() {
            AstKind::Function(_) | AstKind::Class(_) => skip_until = span.end,
            AstKind::ThisExpression(_) => return true,
            AstKind::IdentifierReference(id) if id.name == "arguments" => return true,
            _ => {}
        }
    }
    false
}
