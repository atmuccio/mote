//! `js/default-case`

use crate::prelude::*;

pub struct DefaultCase;

pub static DEFAULT_CASE: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["convention", "conventions"],
    remediation_minutes: 2,
    default_enabled: false,
    description: r#"Flags `switch` statements that have no `default` clause.

**Why:** without `default`, a value that matches no case is silently ignored. Writing the clause
(even one that throws or only holds a comment) shows that the other values were considered.

**Fix:** add a `default:` clause. If falling through is intended, a comment saying `no default`
after the last case documents it and silences this rule.

```js
switch (kind) {         // flagged
  case 'a': runA(); break;
  case 'b': runB(); break;
}

switch (kind) {         // compliant
  case 'a': runA(); break;
  default: throw new Error(`Unknown kind ${kind}`);
}

switch (kind) {         // compliant
  case 'a': runA(); break;
  // no default
}
```
"#,
    ..RuleMeta::new(
        "js/default-case",
        "js",
        "'switch' statements should have a 'default' clause",
    )
};

impl JsRule for DefaultCase {
    fn meta(&self) -> &'static RuleMeta {
        &DEFAULT_CASE
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::SwitchStatement]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::SwitchStatement(switch) = node.kind() else {
            return;
        };
        let Some(last) = switch.cases.last() else {
            return;
        };
        if switch.cases.iter().any(|c| c.test.is_none()) {
            return;
        }
        let tail = Span::new(last.span.start, switch.span.end);
        let comments = ctx.comments();
        let first = comments.partition_point(|c| c.span.start < tail.start);
        let documented = comments
            .get(first..)
            .unwrap_or_default()
            .iter()
            .take_while(|c| c.span.end <= tail.end)
            .any(|c| ctx.text(c.span).to_ascii_lowercase().contains("no default"));
        if documented {
            return;
        }
        ctx.emit(
            Span::new(switch.span.start, switch.span.start + 6),
            "Add a 'default' clause to this 'switch'",
        );
    }
}
