//! `js/no-debugger` — the simplest possible rule: one node type, no conditions.

use crate::prelude::*;

pub struct NoDebugger;

pub static NO_DEBUGGER: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["suspicious"],
    remediation_minutes: 1,
    description: r#"Flags `debugger` statements.

**Why:** a `debugger` statement pauses execution whenever developer tools are open. It is a
debugging aid that should never reach shared code: in production it can freeze a user's page, and in
tests it can hang a run.

**Fix:** delete the statement. Use breakpoints in your editor or browser instead.

```js
function total(items) {
  debugger; // flagged
  return items.reduce((a, b) => a + b, 0);
}
```
"#,
    ..RuleMeta::new(
        "js/no-debugger",
        "js",
        "Debugger statements should be removed",
    )
};

impl JsRule for NoDebugger {
    fn meta(&self) -> &'static RuleMeta {
        &NO_DEBUGGER
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::DebuggerStatement]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let span = node.span();
        // Report the keyword only, not a trailing `;`.
        let keyword = Span::new(span.start, (span.start + 8).min(span.end));
        ctx.emit(keyword, "Remove this 'debugger' statement");
    }
}
