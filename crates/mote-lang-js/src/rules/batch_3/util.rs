//! Private helpers shared by the rules of this batch.

use crate::prelude::*;

/// Span of the trailing `!` of a non-null assertion `expr!` (`None` if the source text does not
/// end with `!`, e.g. for recovered syntax).
pub(super) fn bang_span(ctx: &Ctx<'_>, span: Span) -> Option<Span> {
    if span.end == 0 {
        return None;
    }
    let bang = Span::new(span.end - 1, span.end);
    (ctx.text(bang) == "!").then_some(bang)
}

/// The first non-whitespace character at or after byte offset `from` (comments are not skipped).
pub(super) fn next_char(ctx: &Ctx<'_>, from: u32) -> Option<char> {
    ctx.source()
        .get(from as usize..)?
        .chars()
        .find(|c| !c.is_whitespace())
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

/// Does the file show evidence of React (or Preact, which shares its key and hook rules)?
///
/// JSX alone is not enough: the same syntax compiles to other runtimes (Solid, `@vscode/prompt-tsx`,
/// `hono/jsx`, …) that have no keys or hooks. Evidence is an import from a React-family package
/// (`react`, `react-dom/client`, `preact/hooks`, `mobx-react-lite`, `@fluentui/react-components`,
/// `next/…`), a global `React` reference, or a JSX pragma naming React/Preact.
pub(super) fn is_react_file(ctx: &Ctx<'_>) -> bool {
    if import_sources(ctx).any(is_react_package) {
        return true;
    }
    if ctx
        .scoping()
        .root_unresolved_references()
        .contains_key("React")
    {
        return true;
    }
    // A pragma only matters when there is JSX to compile.
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

/// A module specifier of the React family: one of its `/`, `-`, `.`, `@` separated words is `react`
/// or `preact`, or it is Next.js.
fn is_react_package(source: &str) -> bool {
    // Both words contain `react`: most specifiers are rejected by one substring search.
    source == "next"
        || source.starts_with("next/")
        || (source.contains("react")
            && source
                .split(['/', '-', '.', '@', '_'])
                .any(|word| word == "react" || word == "preact"))
}

/// Is `name` written like a component (`Foo`)?
pub(super) fn is_component_name(name: &str) -> bool {
    name.as_bytes().first().is_some_and(u8::is_ascii_uppercase)
}

/// Is `name` a hook name: `use` followed by an upper-case letter or a digit (`useState`, `use3D`)?
pub(super) fn is_hook_name(name: &str) -> bool {
    name.strip_prefix("use")
        .and_then(|rest| rest.as_bytes().first())
        .is_some_and(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
}

/// Visit the `return` statements that belong to a function body (not those of nested functions),
/// descending only through statements — nested functions only occur inside expressions, so they
/// are never entered.
pub(super) fn for_each_return<'a>(
    statements: &'a [Statement<'a>],
    f: &mut dyn FnMut(&'a ReturnStatement<'a>),
) {
    for stmt in statements {
        return_in_statement(stmt, f);
    }
}

fn return_in_statement<'a>(stmt: &'a Statement<'a>, f: &mut dyn FnMut(&'a ReturnStatement<'a>)) {
    match stmt {
        Statement::ReturnStatement(r) => f(r),
        Statement::BlockStatement(b) => for_each_return(&b.body, f),
        Statement::IfStatement(s) => {
            return_in_statement(&s.consequent, f);
            if let Some(alt) = &s.alternate {
                return_in_statement(alt, f);
            }
        }
        Statement::SwitchStatement(s) => {
            for case in &s.cases {
                for_each_return(&case.consequent, f);
            }
        }
        Statement::TryStatement(s) => {
            for_each_return(&s.block.body, f);
            if let Some(handler) = &s.handler {
                for_each_return(&handler.body.body, f);
            }
            if let Some(finalizer) = &s.finalizer {
                for_each_return(&finalizer.body, f);
            }
        }
        Statement::LabeledStatement(s) => return_in_statement(&s.body, f),
        Statement::ForStatement(s) => return_in_statement(&s.body, f),
        Statement::ForInStatement(s) => return_in_statement(&s.body, f),
        Statement::ForOfStatement(s) => return_in_statement(&s.body, f),
        Statement::WhileStatement(s) => return_in_statement(&s.body, f),
        Statement::DoWhileStatement(s) => return_in_statement(&s.body, f),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_and_component_names() {
        assert!(is_hook_name("useState"));
        assert!(is_hook_name("use3D"));
        assert!(!is_hook_name("use"));
        assert!(!is_hook_name("user"));
        assert!(!is_hook_name("useless"));
        assert!(is_component_name("App"));
        assert!(!is_component_name("app"));
        assert!(!is_component_name(""));
    }
}
