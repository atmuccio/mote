//! `js/not-callable`

use crate::prelude::*;

pub struct NotCallable;

pub static NOT_CALLABLE: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["suspicious"],
    remediation_minutes: 5,
    description: r#"Flags calls and `new` expressions that always throw a `TypeError` because the callee is not a
function or not a constructor: a literal called like a function (`'text'()`, `[a, b]()`,
`null()`), a class declared in the same file called without `new`, and `new` applied to an arrow
function, an `async` function, or a generator declared in the same file.

**Why:** these calls can never work, so the code path that contains them is broken. They usually
come from a missing semicolon — a line starting with `(` or `[` continues the literal on the line
above (`'use strict'` followed by `(function () { … })()` calls the string) — or from a refactoring
that turned a function into a class or an arrow function.

**Fix:** add the missing semicolon, call the class with `new`, or turn the arrow function into a
regular function or a class.

```js
const message = 'hello'
(function () { run(); })();       // flagged: calls the string 'hello'

class Point { constructor(x) { this.x = x; } }
const p = Point(1);               // flagged: classes need `new`

const make = (x) => ({ x });
const q = new make(1);            // flagged: arrow functions are not constructors
```

Only callees whose nature is certain are checked: literals, and classes or functions declared in
the same file whose binding is never reassigned.
"#,
    ..RuleMeta::new(
        "js/not-callable",
        "js",
        "Values that are not functions should not be called",
    )
};

impl JsRule for NotCallable {
    fn meta(&self) -> &'static RuleMeta {
        &NOT_CALLABLE
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CallExpression, AstType::NewExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let (callee, is_new, span) = match node.kind() {
            AstKind::CallExpression(call) => (&call.callee, false, call.span),
            AstKind::NewExpression(new) => (&new.callee, true, new.span),
            _ => return,
        };
        let callee = helpers::unwrap_expr(callee);
        let what = match callee {
            Expression::StringLiteral(_) | Expression::TemplateLiteral(_) => Some("a string"),
            Expression::NumericLiteral(_) | Expression::BigIntLiteral(_) => Some("a number"),
            Expression::BooleanLiteral(_) => Some("a boolean"),
            Expression::NullLiteral(_) => Some("null"),
            Expression::RegExpLiteral(_) => Some("a regular expression"),
            Expression::ArrayExpression(_) => Some("an array"),
            Expression::ObjectExpression(_) => Some("an object"),
            Expression::Identifier(id) if id.name == "undefined" && ctx.is_global_reference(id) => {
                Some("undefined")
            }
            _ => None,
        };
        if let Some(what) = what {
            let text = ctx.text(callee.span());
            let first = text.lines().next().unwrap_or("");
            let shown: String = first.chars().take(30).collect();
            // `'text'\n(…)`: the call starts on the next line, so a semicolon is missing.
            let gap = ctx.text(Span::new(callee.span().end, span.end));
            let continued = !is_new && gap.trim_start().starts_with('(') && {
                let before_paren = &gap[..gap.find('(').unwrap_or(0)];
                before_paren.contains('\n')
            };
            let message = if is_new {
                format!("Do not use 'new' with {shown}: {what} is not a constructor and this throws a TypeError")
            } else if continued {
                format!("Add a semicolon after {shown}: the parenthesis on the next line calls {what}, which throws a TypeError")
            } else {
                format!("Do not call {shown}: {what} is not a function and this throws a TypeError")
            };
            ctx.emit(callee.span(), message);
            return;
        }
        let Expression::Identifier(id) = callee else {
            return;
        };
        let Some(symbol) = ctx.reference_symbol(id) else {
            return;
        };
        // Cheap checks on the declaration first; the reference scan is linear in the uses.
        let candidate = match ctx.symbol_declaration(symbol).kind() {
            AstKind::Class(_) => !is_new,
            AstKind::Function(f) => is_new && (f.r#async || f.generator),
            AstKind::VariableDeclarator(_) => is_new,
            _ => false,
        };
        if !candidate
            || !ctx.scoping().symbol_redeclarations(symbol).is_empty()
            || ctx.symbol_flags(symbol).contains(SymbolFlags::Ambient)
            || ctx.references_of(symbol).any(|r| r.is_write())
        {
            return;
        }
        let name = id.name.as_str();
        match ctx.symbol_declaration(symbol).kind() {
            AstKind::Class(class) if !is_new && !class.declare => {
                ctx.emit(
                    id.span,
                    format!("Call the class '{name}' with 'new': calling a class without it throws a TypeError"),
                );
            }
            AstKind::Function(f) if is_new && f.body.is_some() && (f.r#async || f.generator) => {
                let kind = if f.generator {
                    "a generator"
                } else {
                    "an async function"
                };
                ctx.emit(
                    id.span,
                    format!("Do not use 'new' with '{name}': {kind} is not a constructor and this throws a TypeError"),
                );
            }
            AstKind::VariableDeclarator(d)
                if is_new && d.kind == VariableDeclarationKind::Const =>
            {
                let arrow = matches!(
                    d.init.as_ref().map(helpers::unwrap_expr),
                    Some(Expression::ArrowFunctionExpression(_))
                );
                if arrow && matches!(d.id, BindingPattern::BindingIdentifier(_)) {
                    ctx.emit(
                        id.span,
                        format!("Do not use 'new' with '{name}': arrow functions are not constructors and this throws a TypeError"),
                    );
                }
            }
            _ => {}
        }
    }
}
