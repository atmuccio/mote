//! Private helpers shared by the batch 1 rules.

use crate::prelude::*;
use oxc_span::ContentEq;

/// Is `expr` free of side effects, so that two occurrences of it are guaranteed to evaluate the
/// same way? Identifiers, `this`, primitive literals, member accesses, and operators over those.
/// Calls, `new`, assignments, updates, `await`, `yield`, and regular expressions (a new object
/// each time) are not.
pub(super) fn is_pure(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::Identifier(_)
        | Expression::ThisExpression(_)
        | Expression::StringLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BigIntLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_) => true,
        Expression::TemplateLiteral(t) => t.expressions.iter().all(is_pure),
        Expression::StaticMemberExpression(m) => is_pure(&m.object),
        Expression::PrivateFieldExpression(m) => is_pure(&m.object),
        Expression::ComputedMemberExpression(m) => is_pure(&m.object) && is_pure(&m.expression),
        Expression::ChainExpression(c) => match &c.expression {
            ChainElement::StaticMemberExpression(m) => is_pure(&m.object),
            ChainElement::PrivateFieldExpression(m) => is_pure(&m.object),
            ChainElement::ComputedMemberExpression(m) => {
                is_pure(&m.object) && is_pure(&m.expression)
            }
            ChainElement::TSNonNullExpression(e) => is_pure(&e.expression),
            ChainElement::CallExpression(_) => false,
        },
        Expression::UnaryExpression(u) => {
            u.operator != UnaryOperator::Delete && is_pure(&u.argument)
        }
        Expression::BinaryExpression(b) => is_pure(&b.left) && is_pure(&b.right),
        Expression::PrivateInExpression(p) => is_pure(&p.right),
        Expression::LogicalExpression(l) => is_pure(&l.left) && is_pure(&l.right),
        Expression::ConditionalExpression(c) => {
            is_pure(&c.test) && is_pure(&c.consequent) && is_pure(&c.alternate)
        }
        Expression::TSAsExpression(e) => is_pure(&e.expression),
        Expression::TSSatisfiesExpression(e) => is_pure(&e.expression),
        Expression::TSNonNullExpression(e) => is_pure(&e.expression),
        Expression::TSTypeAssertion(e) => is_pure(&e.expression),
        _ => false,
    }
}

/// Two side-effect-free expressions with the same structure (spans and formatting ignored).
pub(super) fn same_pure_expr(a: &Expression<'_>, b: &Expression<'_>) -> bool {
    // The structural comparison first: it fails at once for operands of different shapes, where
    // `is_pure` walks the whole of `a` (each link of a long comparison chain did).
    a.content_eq(b) && is_pure(a)
}

/// Does the statement never complete normally, i.e. always leave through `return`, `throw`,
/// `break`, or `continue`? Conservative: loops, `switch`, and labeled statements count as
/// completing normally.
pub(super) fn always_exits(stmt: &Statement<'_>) -> bool {
    match stmt {
        Statement::ReturnStatement(_)
        | Statement::ThrowStatement(_)
        | Statement::BreakStatement(_)
        | Statement::ContinueStatement(_) => true,
        Statement::BlockStatement(b) => list_exits(&b.body),
        Statement::IfStatement(s) => s
            .alternate
            .as_ref()
            .is_some_and(|alt| always_exits(&s.consequent) && always_exits(alt)),
        Statement::TryStatement(t) => {
            t.finalizer.as_ref().is_some_and(|f| list_exits(&f.body))
                || (list_exits(&t.block.body)
                    && t.handler.as_ref().is_none_or(|h| list_exits(&h.body.body)))
        }
        _ => false,
    }
}

/// Does a statement list never complete normally (some statement in it always exits)?
pub(super) fn list_exits(stmts: &[Statement<'_>]) -> bool {
    stmts.iter().any(always_exits)
}

/// Does the statement always end by throwing? Like [`always_exits`], but only `throw` counts.
pub(super) fn always_throws(stmt: &Statement<'_>) -> bool {
    match stmt {
        Statement::ThrowStatement(_) => true,
        Statement::BlockStatement(b) => list_throws(&b.body),
        Statement::IfStatement(s) => s
            .alternate
            .as_ref()
            .is_some_and(|alt| always_throws(&s.consequent) && always_throws(alt)),
        Statement::TryStatement(t) => {
            t.finalizer.as_ref().is_some_and(|f| list_throws(&f.body))
                || (list_throws(&t.block.body)
                    && t.handler.as_ref().is_none_or(|h| list_throws(&h.body.body)))
        }
        _ => false,
    }
}

/// Does a statement list always end by throwing (some statement in it always throws)?
pub(super) fn list_throws(stmts: &[Statement<'_>]) -> bool {
    stmts.iter().any(always_throws)
}

/// Which kinds of `return` a function body contains: `(with a value, without a value)`. Only
/// statements are visited (nested functions live in expressions or declarations and are skipped),
/// so the cost is bounded by the size of the function body.
pub(super) fn return_kinds(stmts: &[Statement<'_>]) -> (bool, bool) {
    let mut kinds = (false, false);
    for stmt in stmts {
        collect_returns(stmt, &mut kinds);
        if kinds.0 && kinds.1 {
            break;
        }
    }
    kinds
}

fn collect_returns(stmt: &Statement<'_>, kinds: &mut (bool, bool)) {
    let list = |stmts: &[Statement<'_>], kinds: &mut (bool, bool)| {
        for s in stmts {
            collect_returns(s, kinds);
        }
    };
    match stmt {
        Statement::ReturnStatement(r) => {
            if r.argument.is_some() {
                kinds.0 = true;
            } else {
                kinds.1 = true;
            }
        }
        Statement::BlockStatement(b) => list(&b.body, kinds),
        Statement::IfStatement(s) => {
            collect_returns(&s.consequent, kinds);
            if let Some(alt) = &s.alternate {
                collect_returns(alt, kinds);
            }
        }
        Statement::ForStatement(s) => collect_returns(&s.body, kinds),
        Statement::ForInStatement(s) => collect_returns(&s.body, kinds),
        Statement::ForOfStatement(s) => collect_returns(&s.body, kinds),
        Statement::WhileStatement(s) => collect_returns(&s.body, kinds),
        Statement::DoWhileStatement(s) => collect_returns(&s.body, kinds),
        Statement::LabeledStatement(s) => collect_returns(&s.body, kinds),
        Statement::WithStatement(s) => collect_returns(&s.body, kinds),
        Statement::SwitchStatement(s) => {
            for case in &s.cases {
                list(&case.consequent, kinds);
            }
        }
        Statement::TryStatement(t) => {
            list(&t.block.body, kinds);
            if let Some(h) = &t.handler {
                list(&h.body.body, kinds);
            }
            if let Some(f) = &t.finalizer {
                list(&f.body, kinds);
            }
        }
        _ => {}
    }
}

/// Span of the leading keyword of a statement or expression (`return`, `async`, …) when the source
/// really starts with it; otherwise the whole span.
pub(super) fn keyword_span(ctx: &Ctx<'_>, span: Span, keyword: &str) -> Span {
    if ctx.text(span).starts_with(keyword) {
        Span::new(span.start, span.start + keyword.len() as u32)
    } else {
        span
    }
}

/// Count the `(` characters immediately before `offset`, skipping whitespace.
pub(super) fn open_parens_before(ctx: &Ctx<'_>, offset: u32) -> usize {
    let Some(before) = ctx.source().get(..offset as usize) else {
        return 0;
    };
    let mut count = 0;
    for c in before.chars().rev() {
        match c {
            '(' => count += 1,
            c if c.is_whitespace() => {}
            _ => break,
        }
    }
    count
}

/// Shorten a source excerpt for a message: first line, at most 40 characters.
pub(super) fn excerpt(text: &str) -> String {
    let line = text.lines().next().unwrap_or("");
    let mut out: String = line.chars().take(40).collect();
    if out.len() < text.len() {
        out.push('…');
    }
    out
}
