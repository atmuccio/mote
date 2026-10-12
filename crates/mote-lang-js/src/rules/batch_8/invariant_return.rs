//! `js/invariant-return`

use super::util::{self, Constant};
use crate::prelude::*;

pub struct InvariantReturn;

pub static INVARIANT_RETURN: RuleMeta = RuleMeta {
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["suspicious", "confusing"],
    remediation_minutes: 10,
    description: r#"Flags a function with several `return` statements that all return the same literal value
(`true`, `0`, `'ok'`, `null`, …) and that cannot end without one of them.

**Why:** separate `return` statements suggest that the function answers differently depending on
the path taken, but here every path gives the same answer, so callers testing the result test
nothing. This usually means an edit was left half done (one branch should return `false`), or the
function no longer needs a result at all.

**Fix:** return the value that each branch really means, or make the function return nothing and
update its callers.

```js
function validate(order) {          // flagged: every path returns true
  if (!order.items.length) {
    showError('empty order');
    return true;
  }
  return true;
}

function validate(order) {          // compliant
  if (!order.items.length) {
    showError('empty order');
    return false;
  }
  return true;
}
```

Only function declarations and functions assigned to a variable are checked, and only when every
use of them in the file is a call whose result is not handed on: methods, callbacks, functions passed
around as values, and functions whose result a caller returns often give a fixed value because the
calling code expects it (`return false` to cancel an event, `return true` to keep a visitor
going). Not reported: functions that can reach their end without a `return`, `return undefined`,
generators, and functions with a single `return`.
"#,
    ..RuleMeta::new(
        "js/invariant-return",
        "js",
        "A function should not return the same value on every path",
    )
};

impl JsRule for InvariantReturn {
    fn meta(&self) -> &'static RuleMeta {
        &INVARIANT_RETURN
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::Function, AstType::ArrowFunctionExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let kind = node.kind();
        if let AstKind::Function(f) = kind {
            if f.generator || f.declare {
                return;
            }
        }
        let Some(statements) = util::body_statements(&kind) else {
            return;
        };
        // Cheap first: the body must end with `return <literal>`.
        let Some(Statement::ReturnStatement(last)) = statements.last() else {
            return;
        };
        let Some(value) = last
            .argument
            .as_ref()
            .and_then(|a| util::constant_of(ctx, a))
        else {
            return;
        };
        if value == Constant::Undefined {
            return;
        }
        let Some((name_span, symbol)) = eligible(ctx, node) else {
            return;
        };
        if !only_called(ctx, symbol) {
            return;
        }
        let mut returns: Vec<Span> = Vec::new();
        for inner in util::own_nodes(ctx, node) {
            let AstKind::ReturnStatement(ret) = inner.kind() else {
                continue;
            };
            let same = ret
                .argument
                .as_ref()
                .and_then(|a| util::constant_of(ctx, a))
                .is_some_and(|c| c == value);
            if !same {
                return;
            }
            returns.push(ret.span);
        }
        if returns.len() < 2 {
            return;
        }
        let mut emitted = ctx.emit(
            name_span,
            format!(
                "Every path of this function returns {}: return a meaningful value or nothing",
                value.describe()
            ),
        );
        for span in returns {
            emitted = emitted.secondary(span, "Same value");
        }
    }
}

/// A function declaration, or a function stored in a variable: the span of its name and its
/// symbol.
fn eligible(ctx: &Ctx<'_>, node: &AstNode<'_>) -> Option<(Span, SymbolId)> {
    let id = match node.kind() {
        AstKind::Function(f) if f.is_declaration() => f.id.as_ref()?,
        AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => {
            match ctx.outer_parent(node).map(AstNode::kind) {
                Some(AstKind::VariableDeclarator(d)) => match &d.id {
                    BindingPattern::BindingIdentifier(id) => id,
                    _ => return None,
                },
                _ => return None,
            }
        }
        _ => return None,
    };
    Some((id.span, ctx.binding_symbol(id)?))
}

/// Is the function only ever called by name, with results that are not handed on? A function
/// passed around as a value (a handler, a replacement for a library method) or whose result a
/// caller returns may be answering a protocol that fixes the value.
fn only_called(ctx: &Ctx<'_>, symbol: SymbolId) -> bool {
    if !ctx.scoping().symbol_redeclarations(symbol).is_empty() {
        return false;
    }
    ctx.references_of(symbol).all(|reference| {
        if reference.is_type() {
            return true;
        }
        let node = ctx.node(reference.node_id());
        let Some(call) = ctx.outer_parent(node) else {
            return false;
        };
        let AstKind::CallExpression(c) = call.kind() else {
            return false;
        };
        if !helpers::unwrap_expr(&c.callee)
            .span()
            .contains_inclusive(node.span())
        {
            return false;
        }
        let mut used = call;
        while let Some(parent) = ctx.outer_parent(used) {
            if !matches!(parent.kind(), AstKind::AwaitExpression(_)) {
                break;
            }
            used = parent;
        }
        match ctx.outer_parent(used).map(AstNode::kind) {
            Some(AstKind::ReturnStatement(_)) => false,
            Some(AstKind::ExpressionStatement(_)) => !is_arrow_expression_body(ctx, used),
            _ => true,
        }
    })
}

/// Is the call the expression body of an arrow function (`() => f()`)?
fn is_arrow_expression_body(ctx: &Ctx<'_>, call: &AstNode<'_>) -> bool {
    let Some(stmt) = ctx.outer_parent(call) else {
        return false;
    };
    let Some(body) = ctx.parent(stmt) else {
        return false;
    };
    matches!(body.kind(), AstKind::FunctionBody(_))
        && matches!(ctx.parent_kind(body), Some(AstKind::ArrowFunctionExpression(a)) if a.expression)
}
