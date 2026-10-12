//! `js/default-case-last` (off by default; part of the `conventions` set)

use crate::prelude::*;

pub struct DefaultCaseLast;

pub static DEFAULT_CASE_LAST: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Incubating,
    default_enabled: false,
    tags: &["conventions"],
    remediation_minutes: 2,
    description: r#"Flags a `default` clause that is not the last clause of its `switch`.

**Why:** readers expect `default` at the end, as the case that handles "everything else". A
`default` in the middle still only runs when no other case matches, but it can fall through into
the cases below it, so the control flow differs from what its position suggests, and a case
added after it is easy to overlook.

**Fix:** move the `default` clause to the end (adding a `break` where it relied on falling
through).

```js
switch (kind) {
  default: handleOther(); break;     // flagged
  case 'a': handleA(); break;
}

switch (kind) {
  case 'a': handleA(); break;
  default: handleOther();            // compliant
}
```

Off by default: part of the opt-in `conventions` set.
"#,
    ..RuleMeta::new(
        "js/default-case-last",
        "js",
        "The 'default' clause should be the last clause of a 'switch'",
    )
};

impl JsRule for DefaultCaseLast {
    fn meta(&self) -> &'static RuleMeta {
        &DEFAULT_CASE_LAST
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::SwitchStatement]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::SwitchStatement(switch) = node.kind() else {
            return;
        };
        let Some((last, rest)) = switch.cases.split_last() else {
            return;
        };
        if last.test.is_none() {
            return;
        }
        if let Some(default) = rest.iter().find(|c| c.test.is_none()) {
            ctx.emit(
                Span::new(default.span.start, default.span.start + 7),
                "Move the 'default' clause to the end of the 'switch'",
            );
        }
    }
}
