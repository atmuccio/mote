//! `js/no-labels`

use crate::prelude::*;

pub struct NoLabels;

pub static NO_LABELS: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["confusing", "control-flow"],
    remediation_minutes: 10,
    description: r#"Flags labeled statements (`outer: for (…)`), on loops and on any other statement.

**Why:** labels exist for `break label` and `continue label` jumps across several levels of
nesting. The reader has to find the label to know where execution continues, and the nesting that
made the jump necessary is a readability problem of its own. A label on a block or an `if`, where
`break label` acts as a local `goto`, is rarer still and surprises most readers.

**Fix:** move the nested loops into a function and `return` from it, or use array methods that
stop early (`some`, `find`), or restructure the condition.

```js
search:                                  // flagged
for (const row of rows) {
  for (const cell of row) {
    if (cell.matches(query)) {
      found = cell;
      break search;
    }
  }
}

found = findFirstMatch(rows, query);     // compliant: the inner loops `return cell`
```

Not reported: `name: Identifier;` in JavaScript files, which is a type annotation written in the
wrong language rather than a label.
"#,
    ..RuleMeta::new(
        "js/no-labels",
        "js",
        "Labeled statements should not be used",
    )
};

impl JsRule for NoLabels {
    fn meta(&self) -> &'static RuleMeta {
        &NO_LABELS
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::LabeledStatement]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::LabeledStatement(stmt) = node.kind() else {
            return;
        };
        // `name: Type;` is a TypeScript field annotation in a JavaScript file, not a label.
        if let Statement::ExpressionStatement(e) = &stmt.body {
            if matches!(e.expression, Expression::Identifier(_)) {
                return;
            }
        }
        let message = if matches!(
            stmt.body,
            Statement::ForStatement(_)
                | Statement::ForInStatement(_)
                | Statement::ForOfStatement(_)
                | Statement::WhileStatement(_)
                | Statement::DoWhileStatement(_)
        ) {
            format!(
                "Remove the label '{}': move the nested loops into a function and return from it",
                stmt.label.name
            )
        } else {
            format!(
                "Remove the label '{}': a jump out of a labeled block is hard to follow",
                stmt.label.name
            )
        };
        ctx.emit(stmt.label.span, message);
    }
}
