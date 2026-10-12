//! Private helpers shared by the batch 8 rules (some copied from `batch_6/util.rs`, whose items
//! are private to that batch).

use crate::prelude::*;

/// The program node of the file.
pub(super) fn program<'a>(ctx: &Ctx<'a>) -> Option<&'a Program<'a>> {
    match ctx.node(NodeId::ROOT).kind() {
        AstKind::Program(p) => Some(p),
        _ => None,
    }
}

/// `span` cut at the end of its first line (for reporting multi-line constructs).
pub(super) fn first_line(ctx: &Ctx<'_>, span: Span) -> Span {
    match ctx.text(span).find('\n') {
        Some(i) => Span::new(span.start, span.start + i as u32),
        None => span,
    }
}

/// Is `a` inside `b`?
pub(super) fn within(a: Span, b: Span) -> bool {
    b.start <= a.start && a.end <= b.end
}

/// The nodes below `root`, in source (pre-)order. The semantic builder numbers nodes in traversal
/// order, so a subtree is the run of ids that follows its root and stays inside the root's span.
pub(super) fn descendants<'a>(
    ctx: &Ctx<'a>,
    root: &AstNode<'a>,
) -> impl Iterator<Item = &'a AstNode<'a>> + 'a {
    let nodes = ctx.nodes();
    let span = root.span();
    let start = root.id().index() + 1;
    (start..nodes.len())
        .map(move |i| nodes.get_node(NodeId::new(i)))
        .take_while(move |n| {
            let s = n.span();
            s.start < span.end && s.end <= span.end
        })
}

/// Test-framework globals that register suites and cases.
pub(super) const TEST_FUNCTIONS: &[&str] =
    &["describe", "it", "test", "context", "suite", "specify"];

/// The identifier is a test API: an unresolved global or an import (from Jest, Vitest, Mocha, …).
pub(super) fn is_test_api(ctx: &Ctx<'_>, id: &IdentifierReference<'_>) -> bool {
    match ctx.reference_symbol(id) {
        None => ctx.is_global_reference(id),
        Some(symbol) => ctx.symbol_flags(symbol).contains(SymbolFlags::Import),
    }
}

/// The root identifier and the member names of a callee chain such as `it.skip.each`
/// (`("it", ["skip", "each"])`). Calls in the chain (`test.each(table)`) are stepped through.
pub(super) fn callee_chain<'b, 'a>(
    callee: &'b Expression<'a>,
) -> Option<(&'b IdentifierReference<'a>, Vec<&'a str>)> {
    let mut names = Vec::new();
    let mut current = helpers::unwrap_expr(callee);
    loop {
        match current {
            Expression::Identifier(id) => {
                names.reverse();
                return Some((id, names));
            }
            Expression::StaticMemberExpression(m) => {
                names.push(m.property.name.as_str());
                current = helpers::unwrap_expr(&m.object);
            }
            Expression::CallExpression(c) => current = helpers::unwrap_expr(&c.callee),
            Expression::TaggedTemplateExpression(t) => current = helpers::unwrap_expr(&t.tag),
            _ => return None,
        }
    }
}

/// The root identifier of a callee chain, as [`callee_chain`] finds it (`Some` exactly when
/// it is), without collecting the member names.
pub(super) fn callee_root<'b, 'a>(
    callee: &'b Expression<'a>,
) -> Option<&'b IdentifierReference<'a>> {
    let mut current = helpers::unwrap_expr(callee);
    loop {
        match current {
            Expression::Identifier(id) => return Some(id),
            Expression::StaticMemberExpression(m) => current = helpers::unwrap_expr(&m.object),
            Expression::CallExpression(c) => current = helpers::unwrap_expr(&c.callee),
            Expression::TaggedTemplateExpression(t) => current = helpers::unwrap_expr(&t.tag),
            _ => return None,
        }
    }
}

/// Is the file name that of a test (`.test.` or `.spec.` before the extension)?
pub(super) fn has_test_file_name(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.contains(".test.") || name.contains(".spec.")
}

/// A literal value that can be compared for identity: numbers (with sign), strings (and plain
/// templates), booleans, `null`, and the global `undefined`.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Constant<'a> {
    Number(f64),
    String(&'a str),
    Bool(bool),
    Null,
    Undefined,
}

impl Constant<'_> {
    pub(super) fn describe(&self) -> String {
        match self {
            Constant::Number(n) => format!("{n}"),
            Constant::String(s) => format!("'{s}'"),
            Constant::Bool(b) => format!("{b}"),
            Constant::Null => "null".to_string(),
            Constant::Undefined => "undefined".to_string(),
        }
    }
}

/// The constant an expression denotes, if it is a literal.
pub(super) fn constant_of<'a>(ctx: &Ctx<'_>, expr: &Expression<'a>) -> Option<Constant<'a>> {
    let expr = helpers::unwrap_expr(expr);
    match expr {
        Expression::BooleanLiteral(b) => Some(Constant::Bool(b.value)),
        Expression::NullLiteral(_) => Some(Constant::Null),
        Expression::Identifier(_) if ctx.is_global(expr, "undefined") => Some(Constant::Undefined),
        _ => {
            if let Some(s) = helpers::string_value(expr) {
                return Some(Constant::String(s));
            }
            match expr {
                Expression::NumericLiteral(_) | Expression::UnaryExpression(_) => {
                    helpers::number_value(expr).map(Constant::Number)
                }
                _ => None,
            }
        }
    }
}

/// Is the node a function boundary (a function, arrow, class static block or property
/// initializer runs in its own `return` context)?
pub(super) fn is_function_boundary(kind: AstKind<'_>) -> bool {
    matches!(
        kind,
        AstKind::Function(_)
            | AstKind::ArrowFunctionExpression(_)
            | AstKind::StaticBlock(_)
            | AstKind::PropertyDefinition(_)
            | AstKind::AccessorProperty(_)
    )
}

/// The nodes of a function's own body: its descendants minus those of nested functions.
pub(super) fn own_nodes<'a>(ctx: &Ctx<'a>, function: &AstNode<'a>) -> Vec<&'a AstNode<'a>> {
    let mut out = Vec::new();
    let mut skip_until = 0u32;
    for node in descendants(ctx, function) {
        let span = node.span();
        if span.start < skip_until {
            continue;
        }
        if is_function_boundary(node.kind()) {
            skip_until = span.end;
            continue;
        }
        out.push(node);
    }
    out
}

/// The statements of a function's body (`None` for expression-bodied arrows and bodiless
/// declarations).
pub(super) fn body_statements<'a>(kind: &AstKind<'a>) -> Option<&'a [Statement<'a>]> {
    match kind {
        AstKind::Function(f) => f.body.as_ref().map(|b| b.statements.as_slice()),
        AstKind::ArrowFunctionExpression(a) if !a.expression => Some(a.body.statements.as_slice()),
        _ => None,
    }
}
