//! `js/immediately-returned-variable`

use crate::prelude::*;

pub struct ImmediatelyReturnedVariable;

pub static IMMEDIATELY_RETURNED_VARIABLE: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["redundant", "readability"],
    remediation_minutes: 2,
    description: r#"Flags a local variable that is declared with a value and then returned or thrown by the very
next statement, and used nowhere else.

**Why:** the variable adds a name and a line without adding information; the reader has to check
that nothing happens to it in between, only to find that nothing does.

**Fix:** return (or throw) the expression directly.

```js
const label = `${prefix}-${name}`;       // flagged
return label;

const error = new Error('closed');       // flagged
throw error;

return `${prefix}-${name}`;              // compliant
throw new Error('closed');               // compliant
```

Not reported: declarations with a type annotation (in TypeScript the annotation states or checks the
type of the value), destructuring, `using` declarations (the value is disposed of when the block
ends), function, arrow and class expressions (the variable gives them their `name`), variables used
anywhere else, and pairs separated by a comment.
"#,
    ..RuleMeta::new(
        "js/immediately-returned-variable",
        "js",
        "Values should be returned directly rather than through a temporary variable",
    )
};

impl JsRule for ImmediatelyReturnedVariable {
    fn meta(&self) -> &'static RuleMeta {
        &IMMEDIATELY_RETURNED_VARIABLE
    }

    fn types(&self) -> &'static [AstType] {
        &[
            AstType::BlockStatement,
            AstType::FunctionBody,
            AstType::SwitchCase,
        ]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let statements: &[Statement<'a>] = match node.kind() {
            AstKind::BlockStatement(b) => &b.body,
            AstKind::FunctionBody(b) => &b.statements,
            AstKind::SwitchCase(c) => &c.consequent,
            _ => return,
        };
        for pair in statements.windows(2) {
            let [Statement::VariableDeclaration(decl), next] = pair else {
                continue;
            };
            let (returned, verb) = match next {
                Statement::ReturnStatement(r) => match &r.argument {
                    Some(arg) => (arg, "Return"),
                    None => continue,
                },
                Statement::ThrowStatement(t) => (&t.argument, "Throw"),
                _ => continue,
            };
            let Expression::Identifier(returned) = returned else {
                continue;
            };
            if decl.declare
                || matches!(
                    decl.kind,
                    VariableDeclarationKind::Using | VariableDeclarationKind::AwaitUsing
                )
            {
                continue;
            }
            let [declarator] = decl.declarations.as_slice() else {
                continue;
            };
            let BindingPattern::BindingIdentifier(id) = &declarator.id else {
                continue;
            };
            if id.name != returned.name || declarator.type_annotation.is_some() {
                continue;
            }
            let Some(init) = &declarator.init else {
                continue;
            };
            if matches!(
                helpers::unwrap_expr(init),
                Expression::ArrowFunctionExpression(_)
                    | Expression::FunctionExpression(_)
                    | Expression::ClassExpression(_)
            ) {
                continue;
            }
            let (Some(symbol), Some(target)) =
                (ctx.binding_symbol(id), ctx.reference_symbol(returned))
            else {
                continue;
            };
            if symbol != target || ctx.references_of(symbol).count() != 1 {
                continue;
            }
            if ctx.has_comments_in(Span::new(decl.span.end, next.span().start))
                || ctx.has_comments_in(next.span())
            {
                continue;
            }
            ctx.emit(
                id.span,
                format!(
                    "{verb} the value directly instead of storing it in '{}' first",
                    id.name
                ),
            );
        }
    }
}
