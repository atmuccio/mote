//! `js/no-template-curly-in-string`

use crate::prelude::*;

pub struct NoTemplateCurlyInString;

pub static NO_TEMPLATE_CURLY_IN_STRING: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["suspicious"],
    remediation_minutes: 2,
    description: r#"Flags ordinary `'…'` or `"…"` strings that contain a template placeholder such as `${name}`
where `name` is a variable in scope (or `this`).

**Why:** placeholders are only substituted inside backtick template literals. In a quoted string
`${name}` is printed literally, so the message shows `Hello ${name}` instead of the user's name —
a mistake that is easy to make when switching quote styles.

**Fix:** use backticks for the string.

```js
const name = user.name;
const greeting = 'Hello ${name}!';   // flagged
const greeting = `Hello ${name}!`;   // compliant
const pattern = '${workspaceFolder}/out'; // compliant: no such variable, a placeholder syntax
```

Many strings are deliberately written in another placeholder syntax (snippets, configuration
variables such as `${1:label}`, `${env:HOME}`, or `${workspaceFolder}`). To stay precise, a string
is only reported when every placeholder in it is a plain JavaScript expression whose first name is
`this` or a variable in scope, when it has text besides the placeholders (`'${fileName}'` alone
is a token), and when it contains no backtick (code samples). Test and suite titles
(`describe('GET /users/${id}', …)`) are labels and are not reported, nor is a placeholder that
names a variable holding an object, function or class (`${field}` for `const field = new
Field(…)`): substituting it would print `[object Object]`, so the string uses another placeholder
syntax that happens to share the name.
"#,
    ..RuleMeta::new(
        "js/no-template-curly-in-string",
        "js",
        "Template placeholders should not appear in ordinary strings",
    )
};

impl JsRule for NoTemplateCurlyInString {
    fn meta(&self) -> &'static RuleMeta {
        &NO_TEMPLATE_CURLY_IN_STRING
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::StringLiteral]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::StringLiteral(lit) = node.kind() else {
            return;
        };
        let value = lit.value.as_str();
        // A one-byte search rejects most strings before the substring search.
        if !value.contains('$') || !value.contains("${") {
            return;
        }
        if matches!(
            ctx.parent_kind(node),
            Some(
                AstKind::TSLiteralType(_)
                    | AstKind::ImportDeclaration(_)
                    | AstKind::ExportAllDeclaration(_)
                    | AstKind::ExportNamedDeclaration(_)
                    | AstKind::ImportExpression(_)
            )
        ) {
            return;
        }
        // Code samples embedding real template literals (`'x = `${y}`'`) are data.
        if value.contains('`') {
            return;
        }
        // A test title documenting a route template (`describe('GET /users/${id}', …)`) is only
        // a label.
        if is_test_title(ctx, node) {
            return;
        }
        let scope = node.scope_id();
        let mut first: Option<&str> = None;
        let mut has_text = false;
        let mut rest = value;
        while let Some(start) = rest.find("${") {
            has_text |= !rest.get(..start).unwrap_or("").trim().is_empty();
            let after = rest.get(start + 2..).unwrap_or("");
            let Some(end) = after.find('}') else {
                return;
            };
            let inner = after.get(..end).unwrap_or("").trim();
            // Every placeholder must read as JavaScript naming something in scope; otherwise the
            // string uses another placeholder syntax (`${1:label}`, `${env:HOME}`, `${src}`).
            let known = expression_root(inner).is_some_and(|root| {
                root == "this"
                    || ctx
                        .scoping()
                        .find_binding(scope, root.into())
                        .is_some_and(|s| {
                            ctx.symbol_flags(s).is_value()
                                && !(inner == root && prints_as_object(ctx, s))
                        })
            });
            if !known {
                return;
            }
            first.get_or_insert(inner);
            rest = after.get(end + 1..).unwrap_or("");
        }
        has_text |= !rest.trim().is_empty();
        // A string that is nothing but placeholders (`'${fileName}'`) is a token by design.
        let (Some(inner), true) = (first, has_text) else {
            return;
        };
        ctx.emit(
            lit.span,
            format!(
                "Use a template literal (backticks): '${{{}}}' is not substituted in a quoted string",
                truncate(inner)
            ),
        );
    }
}

/// Is the string the title of a test or suite: the first argument of `describe`, `it`, `test`,
/// `context`, `suite`, `specify` (or their `.only` / `.skip` / `.todo` forms)?
fn is_test_title(ctx: &Ctx<'_>, literal: &AstNode<'_>) -> bool {
    let Some(AstKind::CallExpression(call)) = ctx.parent_kind(literal) else {
        return false;
    };
    if call.arguments.first().map(GetSpan::span) != Some(literal.span()) {
        return false;
    }
    let name = match helpers::unwrap_expr(&call.callee) {
        Expression::Identifier(id) => id.name.as_str(),
        Expression::StaticMemberExpression(m)
            if matches!(m.property.name.as_str(), "only" | "skip" | "todo") =>
        {
            match helpers::unwrap_expr(&m.object) {
                Expression::Identifier(id) => id.name.as_str(),
                _ => return false,
            }
        }
        _ => return false,
    };
    matches!(
        name,
        "describe" | "it" | "test" | "context" | "suite" | "specify"
    )
}

/// The variable holds an object, function or class (`const field = new Field(…)`, `= {…}`):
/// interpolating it would print `[object Object]` or source text, so a `${field}` naming it
/// belongs to another placeholder syntax that happens to share the name.
fn prints_as_object(ctx: &Ctx<'_>, symbol: SymbolId) -> bool {
    match ctx.symbol_declaration(symbol).kind() {
        AstKind::VariableDeclarator(d) => matches!(
            d.init.as_ref().map(helpers::unwrap_expr),
            Some(
                Expression::NewExpression(_)
                    | Expression::ObjectExpression(_)
                    | Expression::FunctionExpression(_)
                    | Expression::ArrowFunctionExpression(_)
                    | Expression::ClassExpression(_)
            )
        ),
        AstKind::Function(_) | AstKind::Class(_) => true,
        _ => false,
    }
}

/// The leading name of a placeholder that reads like a JavaScript expression (`name`,
/// `user.name`, `items[0]`, `fmt(x)`, `a + b`), or `None` for other placeholder syntaxes
/// (`1:label`, `env:HOME`, `config.x:y`, empty).
fn expression_root(inner: &str) -> Option<&str> {
    let end = inner
        .char_indices()
        .find(|&(i, c)| {
            !(c == '_' || c == '$' || c.is_alphabetic() || (i > 0 && c.is_ascii_digit()))
        })
        .map_or(inner.len(), |(i, _)| i);
    if end == 0 {
        return None;
    }
    let root = inner.get(..end)?;
    let tail = inner.get(end..)?;
    let allowed = |c: char| {
        c.is_alphanumeric()
            || matches!(
                c,
                '_' | '$'
                    | '.'
                    | '?'
                    | '['
                    | ']'
                    | '('
                    | ')'
                    | ' '
                    | ','
                    | '\''
                    | '"'
                    | '+'
                    | '-'
                    | '*'
                    | '/'
                    | '!'
                    | '='
            )
    };
    if tail.contains(|c: char| !allowed(c)) || tail.starts_with('?') && !tail.starts_with("?.") {
        return None;
    }
    Some(root)
}

fn truncate(text: &str) -> String {
    text.chars().take(30).collect()
}
