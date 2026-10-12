//! Private helpers shared by the batch 6 rules.

use crate::prelude::*;

/// Is `name` referenced as an unresolved global anywhere in the file?
pub(crate) fn uses_global(ctx: &Ctx<'_>, name: &str) -> bool {
    ctx.scoping()
        .root_unresolved_references()
        .contains_key(name)
}

/// The program node of the file.
pub(crate) fn program<'a>(ctx: &Ctx<'a>) -> Option<&'a Program<'a>> {
    match ctx.node(NodeId::ROOT).kind() {
        AstKind::Program(p) => Some(p),
        _ => None,
    }
}

/// Module specifiers of the file's top-level `import` declarations.
pub(crate) fn import_sources<'a>(ctx: &Ctx<'a>) -> impl Iterator<Item = &'a str> + 'a {
    program(ctx)
        .into_iter()
        .flat_map(|p| p.body.iter())
        .filter_map(|stmt| match stmt {
            Statement::ImportDeclaration(import) => Some(import.source.value.as_str()),
            _ => None,
        })
}

/// Does the file show evidence of React (or Preact)? An import from a React-family package or a
/// global `React` reference.
pub(crate) fn is_react_file(ctx: &Ctx<'_>) -> bool {
    import_sources(ctx).any(is_react_package) || uses_global(ctx, "React")
}

/// A module specifier of the React family (`react`, `react-dom/client`, `preact/hooks`,
/// `mobx-react-lite`, `next/…`).
pub(crate) fn is_react_package(source: &str) -> bool {
    // Both words contain `react`: most specifiers are rejected by one substring search.
    source == "next"
        || source.starts_with("next/")
        || (source.contains("react")
            && source
                .split(['/', '-', '.', '@', '_'])
                .any(|word| word == "react" || word == "preact"))
}

/// `span` cut at the end of its first line (for reporting multi-line expressions).
pub(crate) fn first_line(ctx: &Ctx<'_>, span: Span) -> Span {
    match ctx.text(span).find('\n') {
        Some(i) => Span::new(span.start, span.start + i as u32),
        None => span,
    }
}

/// Is `a` inside `b`?
pub(crate) fn within(a: Span, b: Span) -> bool {
    b.start <= a.start && a.end <= b.end
}

/// An arrow function's expression body is represented as an expression statement.
pub(crate) fn is_arrow_expression_body(ctx: &Ctx<'_>, stmt: &AstNode<'_>) -> bool {
    let Some(body) = ctx.parent(stmt) else {
        return false;
    };
    matches!(body.kind(), AstKind::FunctionBody(_))
        && matches!(ctx.parent_kind(body), Some(AstKind::ArrowFunctionExpression(a)) if a.expression)
}

/// The callee resolves to an `async` (non-generator) function declared in this file and never
/// reassigned: a function declaration, or a `const`/`let` holding an `async` arrow or function
/// expression.
pub(crate) fn is_local_async_function(ctx: &Ctx<'_>, callee: &IdentifierReference<'_>) -> bool {
    let Some(symbol) = ctx.reference_symbol(callee) else {
        return false;
    };
    let is_async = match ctx.symbol_declaration(symbol).kind() {
        AstKind::Function(f) => f.r#async && !f.generator && f.body.is_some(),
        AstKind::VariableDeclarator(d) => {
            d.kind != VariableDeclarationKind::Var
                && matches!(d.id, BindingPattern::BindingIdentifier(_))
                && match d.init.as_ref().map(helpers::unwrap_expr) {
                    Some(Expression::ArrowFunctionExpression(a)) => a.r#async,
                    Some(Expression::FunctionExpression(f)) => f.r#async && !f.generator,
                    _ => false,
                }
        }
        _ => false,
    };
    // The reference scan comes last: it is linear in the number of uses of the function.
    is_async
        && !ctx.symbol_flags(symbol).contains(SymbolFlags::Ambient)
        && ctx.scoping().symbol_redeclarations(symbol).is_empty()
        && !ctx.references_of(symbol).any(|r| r.is_write())
}

/// The nodes below `root`, in source (pre-)order. The semantic builder numbers nodes in traversal
/// order, so a subtree is the run of ids that follows its root and stays inside the root's span.
pub(crate) fn descendants<'a>(
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

/// Does any unresolved reference named `arguments` lie inside `span`?
pub(crate) fn uses_arguments_in(ctx: &Ctx<'_>, span: Span) -> bool {
    let scoping = ctx.scoping();
    scoping
        .root_unresolved_references()
        .get("arguments")
        .is_some_and(|ids| {
            ids.iter().any(|&id| {
                let at = ctx.node(scoping.get_reference(id).node_id()).span().start;
                span.start <= at && at < span.end
            })
        })
}

/// What an expression visibly is, without type information.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ValueKind {
    String,
    Array,
}

/// A variable or parameter declared with a `string` / array type, or initialized with a literal
/// and never reassigned.
pub(crate) fn identifier_kind(ctx: &Ctx<'_>, id: &IdentifierReference<'_>) -> Option<ValueKind> {
    let symbol = ctx.reference_symbol(id)?;
    if !ctx.scoping().symbol_redeclarations(symbol).is_empty() {
        return None;
    }
    let reassigned = ctx.references_of(symbol).any(|r| r.is_write());
    match ctx.symbol_declaration(symbol).kind() {
        AstKind::VariableDeclarator(d) => {
            if !matches!(d.id, BindingPattern::BindingIdentifier(_)) {
                return None;
            }
            if let Some(annotation) = &d.type_annotation {
                return type_kind(&annotation.type_annotation);
            }
            if reassigned {
                return None;
            }
            match d.init.as_ref().map(helpers::unwrap_expr)? {
                Expression::StringLiteral(_) | Expression::TemplateLiteral(_) => {
                    Some(ValueKind::String)
                }
                Expression::ArrayExpression(_) => Some(ValueKind::Array),
                _ => None,
            }
        }
        AstKind::FormalParameter(p) => {
            if !matches!(p.pattern, BindingPattern::BindingIdentifier(_)) {
                return None;
            }
            type_kind(&p.type_annotation.as_ref()?.type_annotation)
        }
        _ => None,
    }
}

/// `string`, `T[]`, `readonly T[]`, `Array<T>`, `ReadonlyArray<T>`.
pub(crate) fn type_kind(ty: &TSType<'_>) -> Option<ValueKind> {
    match ty {
        TSType::TSStringKeyword(_) => Some(ValueKind::String),
        TSType::TSArrayType(_) => Some(ValueKind::Array),
        TSType::TSTypeOperatorType(op) => match &op.type_annotation {
            TSType::TSArrayType(_) => Some(ValueKind::Array),
            _ => None,
        },
        TSType::TSTypeReference(r) => match &r.type_name {
            TSTypeName::IdentifierReference(name)
                if matches!(name.name.as_str(), "Array" | "ReadonlyArray") =>
            {
                Some(ValueKind::Array)
            }
            _ => None,
        },
        _ => None,
    }
}
