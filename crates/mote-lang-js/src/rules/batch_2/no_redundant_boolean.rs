//! `js/no-redundant-boolean`

use crate::prelude::*;

pub struct NoRedundantBoolean;

pub static NO_REDUNDANT_BOOLEAN: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["redundant"],
    remediation_minutes: 1,
    description: r#"Flags boolean literals that add nothing: a ternary that picks between `true` and
`false` (`c ? true : false`), an `if` that returns `true` in one branch and `false` in the other
(with an `else`, or with the second `return` right after the `if`), and a comparison of a boolean
value with `true` or `false` (`(a > b) === true`).

**Why:** the literal restates what the expression already is and makes the reader stop to check
for a trick.

**Fix:** use the condition directly (`!!c` or `Boolean(c)` when it is not already a boolean) or
negate it (`c ? false : true` is `!c`, `x === false` is `!x`); return the condition instead of
`if (c) { return true; } return false;`.

Comparisons are only reported when the other operand is certainly a boolean: a comparison, a
negation, `instanceof`/`in`, a `Boolean(…)`/`Array.isArray(…)` call, a combination of those with
`&&`/`||`, or a variable or parameter declared as `boolean` or initialized from such an
expression. `flag === true` where `flag` may also be `undefined` is a real check and is left
alone, and so is a variable that is also compared with the other boolean, `null`, or `undefined`
(a tri-state value, whatever its declared type).

The last `if` of a chain of guards that all return the same literal is not reported
(`if (a.x !== b.x) return false; if (a.y !== b.y) return false; return true;`, typical of equality
checks), where keeping the parallel structure is clearer; declarations between the guards and
`else if` guards count as part of the chain. After a guard returning the opposite literal the last
`if` is reported.

```ts
const visible = count > 0 ? true : false;   // flagged
if (isReady(x) && (a > b) === true) {}      // flagged
function isEmpty(list) {
  if (list.length === 0) {                  // flagged
    return true;
  }
  return false;
}

const visible = count > 0;                  // compliant
if (options.enabled === true) {}            // compliant: the type is unknown
function isEmpty(list) {
  return list.length === 0;                 // compliant
}
```
"#,
    ..RuleMeta::new(
        "js/no-redundant-boolean",
        "js",
        "Boolean literals should not be used redundantly",
    )
};

fn bool_literal(expr: &Expression<'_>) -> Option<bool> {
    match helpers::unwrap_expr(expr) {
        Expression::BooleanLiteral(b) => Some(b.value),
        _ => None,
    }
}

/// Is `expr` certainly a boolean value? `depth` bounds the look-through into declarations.
fn is_boolean(ctx: &Ctx<'_>, expr: &Expression<'_>, depth: u8) -> bool {
    is_boolean_nested(ctx, expr, depth, 0)
}

fn is_boolean_nested(ctx: &Ctx<'_>, expr: &Expression<'_>, depth: u8, nesting: u8) -> bool {
    if nesting > 24 {
        return false;
    }
    match helpers::unwrap_expr(expr) {
        Expression::BooleanLiteral(_) | Expression::PrivateInExpression(_) => true,
        Expression::UnaryExpression(u) => {
            matches!(
                u.operator,
                UnaryOperator::LogicalNot | UnaryOperator::Delete
            )
        }
        Expression::BinaryExpression(b) => {
            b.operator.is_equality()
                || b.operator.is_compare()
                || matches!(b.operator, BinaryOperator::Instanceof | BinaryOperator::In)
        }
        Expression::LogicalExpression(l) => {
            !l.operator.is_coalesce()
                && is_boolean_nested(ctx, &l.left, depth, nesting + 1)
                && is_boolean_nested(ctx, &l.right, depth, nesting + 1)
        }
        Expression::CallExpression(call) => {
            if call.optional {
                return false;
            }
            let callee = helpers::unwrap_expr(&call.callee);
            if ctx.is_global(callee, "Boolean") {
                return true;
            }
            match helpers::static_member_names(callee) {
                Some(("Array", "isArray"))
                | Some(("Number", "isNaN" | "isFinite" | "isInteger" | "isSafeInteger"))
                | Some(("Object", "is" | "isFrozen" | "isSealed" | "isExtensible")) => {
                    if let Expression::StaticMemberExpression(m) = callee {
                        let Expression::Identifier(obj) = helpers::unwrap_expr(&m.object) else {
                            return false;
                        };
                        ctx.is_global_reference(obj)
                    } else {
                        false
                    }
                }
                _ => false,
            }
        }
        Expression::Identifier(ident) if depth > 0 => {
            let Some(symbol) = ctx.reference_symbol(ident) else {
                return false;
            };
            declared_boolean(ctx, symbol, depth - 1)
        }
        _ => false,
    }
}

fn is_boolean_type(annotation: Option<&TSTypeAnnotation<'_>>) -> bool {
    annotation.is_some_and(|t| matches!(t.type_annotation, TSType::TSBooleanKeyword(_)))
}

/// A variable or parameter whose declaration guarantees a boolean value.
fn declared_boolean(ctx: &Ctx<'_>, symbol: SymbolId, depth: u8) -> bool {
    if !ctx.scoping().symbol_redeclarations(symbol).is_empty() {
        return false;
    }
    match ctx.symbol_declaration(symbol).kind() {
        AstKind::VariableDeclarator(d) => {
            let BindingPattern::BindingIdentifier(_) = &d.id else {
                return false;
            };
            if d.definite {
                return false;
            }
            let Some(init) = &d.init else {
                return false;
            };
            if is_boolean_type(d.type_annotation.as_deref()) {
                return true;
            }
            if !is_boolean(ctx, init, depth) {
                return false;
            }
            // Later assignments must keep it boolean.
            d.kind == VariableDeclarationKind::Const
                || ctx.references_of(symbol).all(|r| !r.flags().is_write())
        }
        AstKind::FormalParameter(p) => {
            matches!(p.pattern, BindingPattern::BindingIdentifier(_))
                && !p.optional
                && is_boolean_type(p.type_annotation.as_deref())
                && p.initializer
                    .as_ref()
                    .is_none_or(|init| is_boolean(ctx, init, 0))
                && ctx.references_of(symbol).all(|r| !r.flags().is_write())
        }
        _ => false,
    }
}

/// Is the variable also compared with the other boolean literal, `null`, or `undefined`
/// somewhere? Then the code treats it as possibly not a boolean (a tri-state setting, a value
/// from an untyped source) whatever its declaration says.
fn compared_as_tri_state(ctx: &Ctx<'_>, symbol: SymbolId, literal: bool) -> bool {
    ctx.references_of(symbol).any(|reference| {
        let node = ctx.node(reference.node_id());
        let Some(parent) = ctx.outer_parent(node) else {
            return false;
        };
        let AstKind::BinaryExpression(b) = parent.kind() else {
            return false;
        };
        if !b.operator.is_equality() {
            return false;
        }
        let at = node.span().start;
        let other = if b.left.span().start <= at && at < b.left.span().end {
            &b.right
        } else {
            &b.left
        };
        match helpers::unwrap_expr(other) {
            Expression::BooleanLiteral(v) => v.value != literal,
            Expression::NullLiteral(_) => true,
            e => ctx.is_global(e, "undefined"),
        }
    })
}

impl JsRule for NoRedundantBoolean {
    fn meta(&self) -> &'static RuleMeta {
        &NO_REDUNDANT_BOOLEAN
    }

    fn types(&self) -> &'static [AstType] {
        &[
            AstType::BinaryExpression,
            AstType::ConditionalExpression,
            AstType::IfStatement,
        ]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        match node.kind() {
            AstKind::IfStatement(stmt) => check_if_return(node, stmt, ctx),
            AstKind::ConditionalExpression(c) => {
                let (Some(yes), Some(no)) =
                    (bool_literal(&c.consequent), bool_literal(&c.alternate))
                else {
                    return;
                };
                if yes == no {
                    return;
                }
                let message = if yes {
                    if is_boolean(ctx, &c.test, 1) {
                        "Use the condition itself instead of '? true : false'"
                    } else {
                        "Use '!!condition' or 'Boolean(condition)' instead of '? true : false'"
                    }
                } else {
                    "Use '!condition' instead of '? false : true'"
                };
                ctx.emit(c.span, message);
            }
            AstKind::BinaryExpression(b) => {
                if !b.operator.is_equality() {
                    return;
                }
                let (literal, other) = match (bool_literal(&b.left), bool_literal(&b.right)) {
                    (Some(_), Some(_)) | (None, None) => return,
                    (Some(v), None) => (v, &b.right),
                    (None, Some(v)) => (v, &b.left),
                };
                if !is_boolean(ctx, other, 1) {
                    return;
                }
                if let Expression::Identifier(ident) = helpers::unwrap_expr(other) {
                    if ctx
                        .reference_symbol(ident)
                        .is_some_and(|s| compared_as_tri_state(ctx, s, literal))
                    {
                        return;
                    }
                }
                let negated = matches!(
                    b.operator,
                    BinaryOperator::Inequality | BinaryOperator::StrictInequality
                );
                let message = if literal != negated {
                    format!("Remove the redundant comparison with '{literal}'")
                } else {
                    format!("Negate the operand instead of comparing it with '{literal}'")
                };
                ctx.emit(b.span, message);
            }
            _ => {}
        }
    }
}

/// `Some(v)` for `return true;` / `return false;`, possibly as the only statement of a block.
fn returned_literal(stmt: &Statement<'_>) -> Option<bool> {
    let stmt = match stmt {
        Statement::BlockStatement(block) => match block.body.as_slice() {
            [only] => only,
            _ => return None,
        },
        other => other,
    };
    let Statement::ReturnStatement(ret) = stmt else {
        return None;
    };
    bool_literal(ret.argument.as_ref()?)
}

/// The literal returned by every branch of a guard: `if (c) return v;` without `else`, or an
/// `if … else if …` chain without a final `else` whose branches all return `v`.
fn guard_literal(stmt: &Statement<'_>) -> Option<bool> {
    let Statement::IfStatement(guard) = stmt else {
        return None;
    };
    let value = returned_literal(&guard.consequent)?;
    match &guard.alternate {
        None => Some(value),
        Some(alternate) => (guard_literal(alternate) == Some(value)).then_some(value),
    }
}

/// The statement list holding a statement: a function body, a block, or a `case`.
fn statement_list<'a>(parent: &AstNode<'a>) -> Option<&'a [Statement<'a>]> {
    match parent.kind() {
        AstKind::FunctionBody(body) => Some(body.statements.as_slice()),
        AstKind::BlockStatement(block) => Some(block.body.as_slice()),
        AstKind::SwitchCase(case) => Some(case.consequent.as_slice()),
        _ => None,
    }
}

/// `if (c) return true; else return false;` and `if (c) return true; return false;`.
fn check_if_return<'a>(node: &AstNode<'a>, stmt: &'a IfStatement<'a>, ctx: &mut Ctx<'a>) {
    let Some(first) = returned_literal(&stmt.consequent) else {
        return;
    };
    let parent = ctx.parent(node);
    let second = match &stmt.alternate {
        Some(alternate) => {
            // `if (a) return false; else if (b) return false; else return true;`: the last
            // branch of a chain of guards returning the same value reads best like the others.
            if let Some(AstKind::IfStatement(outer)) = parent.map(AstNode::kind) {
                if outer
                    .alternate
                    .as_ref()
                    .is_some_and(|a| a.span() == stmt.span)
                    && returned_literal(&outer.consequent) == Some(first)
                {
                    return;
                }
            }
            returned_literal(alternate)
        }
        None => {
            let Some(list) = parent.and_then(statement_list) else {
                return;
            };
            let Some(index) = list.iter().position(|s| s.span() == stmt.span) else {
                return;
            };
            // The last guard of a chain returning the same value (`if (a.x !== b.x) return
            // false; if (a.y !== b.y) return false; return true;`) reads best like its
            // siblings; declarations between the guards (`const o = other as Point;`) belong to
            // the chain.
            let previous = list
                .get(..index)
                .unwrap_or_default()
                .iter()
                .rev()
                .find(|s| !matches!(s, Statement::VariableDeclaration(_)));
            if previous.and_then(guard_literal) == Some(first) {
                return;
            }
            match list.get(index + 1) {
                Some(Statement::ReturnStatement(ret)) => {
                    ret.argument.as_ref().and_then(bool_literal)
                }
                _ => None,
            }
        }
    };
    if second != Some(!first) {
        return;
    }
    let message = if !first {
        "Return the negated condition instead of returning false and true from an 'if'"
    } else if is_boolean(ctx, &stmt.test, 1) {
        "Return the condition itself instead of returning true and false from an 'if'"
    } else {
        "Return '!!condition' or 'Boolean(condition)' instead of returning true and false from an 'if'"
    };
    ctx.emit(Span::new(stmt.span.start, stmt.span.start + 2), message);
}
