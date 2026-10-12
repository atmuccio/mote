//! `js/no-case-declarations`

use crate::prelude::*;

pub struct NoCaseDeclarations;

pub static NO_CASE_DECLARATIONS: RuleMeta = RuleMeta {
    severity: Severity::Medium,
    tags: &["pitfall", "switch"],
    remediation_minutes: 2,
    description: r#"Flags `let`, `const`, `class`, and function declarations placed directly in a
`case` or `default` clause, without braces around the clause body.

**Why:** all clauses of a `switch` share one block scope. A declaration in one clause is visible
in every other clause, but it is only initialized when its own clause runs: using the name from
another clause throws a `ReferenceError` (temporal dead zone) or sees a function that was never
meant for that branch, and declaring the same name in two clauses is a syntax error.

**Fix:** wrap the clause body in braces so the declaration gets its own scope.

```js
switch (action.type) {
  case 'add':
    const item = action.item;      // flagged
    list.push(item);
    break;
  case 'remove': {
    const index = action.index;    // compliant: the clause has its own block
    list.splice(index, 1);
    break;
  }
}
```
"#,
    ..RuleMeta::new(
        "js/no-case-declarations",
        "js",
        "Lexical declarations in 'case' clauses should be wrapped in a block",
    )
};

impl JsRule for NoCaseDeclarations {
    fn meta(&self) -> &'static RuleMeta {
        &NO_CASE_DECLARATIONS
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::SwitchCase]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::SwitchCase(case) = node.kind() else {
            return;
        };
        for stmt in &case.consequent {
            let (span, keyword, what) = match stmt {
                Statement::VariableDeclaration(decl) => {
                    let keyword = match decl.kind {
                        VariableDeclarationKind::Var => continue,
                        VariableDeclarationKind::Let => "let",
                        VariableDeclarationKind::Const => "const",
                        VariableDeclarationKind::Using => "using",
                        VariableDeclarationKind::AwaitUsing => "await",
                    };
                    (decl.span, keyword, "declaration")
                }
                Statement::ClassDeclaration(class) => {
                    if class.declare {
                        continue;
                    }
                    (class.span, "class", "class declaration")
                }
                Statement::FunctionDeclaration(function) => {
                    if function.declare || function.body.is_none() {
                        continue;
                    }
                    let keyword = if function.r#async {
                        "async"
                    } else {
                        "function"
                    };
                    (function.span, keyword, "function declaration")
                }
                _ => continue,
            };
            let keyword_span = if ctx.text(span).starts_with(keyword) {
                Span::new(span.start, span.start + keyword.len() as u32)
            } else {
                super::util::first_line(ctx, span)
            };
            ctx.emit(
                keyword_span,
                format!("Wrap this 'case' body in braces: the {what} is visible in every clause of the switch"),
            );
        }
    }
}
