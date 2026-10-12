//! Private helpers shared by the batch 4 rules.

use crate::prelude::*;

/// Is `name` referenced as an unresolved global anywhere in the file?
pub(super) fn uses_global(ctx: &Ctx<'_>, name: &str) -> bool {
    ctx.scoping()
        .root_unresolved_references()
        .contains_key(name)
}

/// Does the file keep its top-level bindings private: an ES module, a CommonJS file, or a script
/// that uses `require` / `module` / `exports`? In a classic script, top-level declarations are
/// globals shared with every other script.
pub(super) fn has_private_top_level(ctx: &Ctx<'_>) -> bool {
    ctx.is_module()
        || ctx.source_type().is_commonjs()
        || ctx
            .scoping()
            .root_unresolved_references()
            .contains_key("require")
        || ctx
            .scoping()
            .root_unresolved_references()
            .contains_key("module")
        || ctx
            .scoping()
            .root_unresolved_references()
            .contains_key("exports")
}

/// The program node of the file.
pub(super) fn program<'a>(ctx: &Ctx<'a>) -> Option<&'a Program<'a>> {
    match ctx.node(NodeId::ROOT).kind() {
        AstKind::Program(p) => Some(p),
        _ => None,
    }
}

/// Module specifiers of the file's top-level `import` declarations.
pub(super) fn import_sources<'a>(ctx: &Ctx<'a>) -> impl Iterator<Item = &'a str> + 'a {
    program(ctx)
        .into_iter()
        .flat_map(|p| p.body.iter())
        .filter_map(|stmt| match stmt {
            Statement::ImportDeclaration(import) => Some(import.source.value.as_str()),
            _ => None,
        })
}

/// Does the file contain JSX (an element or a fragment)?
pub(super) fn has_jsx(ctx: &Ctx<'_>) -> bool {
    ctx.file_contains(AstType::JSXElement) || ctx.file_contains(AstType::JSXFragment)
}

/// Does the file show evidence of React (or Preact)? An import from a React-family package, a
/// global `React` reference, or a JSX pragma naming React/Preact. JSX alone is not enough: other
/// runtimes (Solid, prompt renderers, server-side templating) use the same syntax.
pub(super) fn is_react_file(ctx: &Ctx<'_>) -> bool {
    if import_sources(ctx).any(is_react_package) {
        return true;
    }
    if uses_global(ctx, "React") {
        return true;
    }
    has_jsx(ctx)
        && ctx.comments().iter().any(|c| {
            let text = ctx.text(c.span);
            ["@jsxImportSource", "@jsx "].iter().any(|pragma| {
                text.find(pragma).is_some_and(|pos| {
                    let rest = text.get(pos + pragma.len()..).unwrap_or("");
                    let value = rest.split_whitespace().next().unwrap_or("");
                    is_react_package(value) || value.starts_with("React.")
                })
            })
        })
}

/// A module specifier of the React family (`react`, `react-dom/client`, `preact/hooks`,
/// `mobx-react-lite`, `next/…`).
fn is_react_package(source: &str) -> bool {
    // Both words contain `react`: most specifiers are rejected by one substring search.
    source == "next"
        || source.starts_with("next/")
        || (source.contains("react")
            && source
                .split(['/', '-', '.', '@', '_'])
                .any(|word| word == "react" || word == "preact"))
}

/// Is `name` written like a component or class (`Foo`)?
pub(super) fn is_capitalized(name: &str) -> bool {
    name.as_bytes().first().is_some_and(u8::is_ascii_uppercase)
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

/// `span` cut at the end of its first line (for reporting multi-line expressions).
pub(super) fn first_line(ctx: &Ctx<'_>, span: Span) -> Span {
    match ctx.text(span).find('\n') {
        Some(i) => Span::new(span.start, span.start + i as u32),
        None => span,
    }
}

/// Span of the leading keyword of a node when the source really starts with it; otherwise the
/// whole span.
pub(super) fn keyword_span(ctx: &Ctx<'_>, span: Span, keyword: &str) -> Span {
    if ctx.text(span).starts_with(keyword) {
        Span::new(span.start, span.start + keyword.len() as u32)
    } else {
        span
    }
}

/// Does any unresolved reference named `arguments` lie inside `span`?
pub(super) fn uses_arguments_in(ctx: &Ctx<'_>, span: Span) -> bool {
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

/// A short, single-line preview of source text for messages (at most `max` characters).
pub(super) fn preview(text: &str, max: usize) -> String {
    let line = text.lines().next().unwrap_or("");
    let mut out: String = line
        .chars()
        .take(max)
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    if out.len() < text.len() {
        out.push('…');
    }
    out
}
