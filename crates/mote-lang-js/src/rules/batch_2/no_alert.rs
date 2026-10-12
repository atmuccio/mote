//! `js/no-alert`

use crate::prelude::*;

pub struct NoAlert;

pub static NO_ALERT: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["user-experience"],
    remediation_minutes: 5,
    description: r#"Flags calls to the browser's blocking dialogs: `alert(…)`, `confirm(…)`, and
`prompt(…)`, also through `window.`, `self.`, or `globalThis.`.

**Why:** these dialogs freeze the page until dismissed, cannot be styled, are blocked or
suppressed in some contexts, and are often leftover debugging output.

**Fix:** use the application's own notification or modal components, or a logger for debugging.
Local functions or variables named `alert`, `confirm`, or `prompt` are not reported.

```js
alert('Saved!');                  // flagged
if (window.confirm('Delete?')) {} // flagged

notifications.info('Saved!');     // compliant
const ok = await dialog.confirm('Delete?'); // compliant
```
"#,
    ..RuleMeta::new(
        "js/no-alert",
        "js",
        "Browser dialogs 'alert', 'confirm', and 'prompt' should not be used",
    )
};

fn is_dialog(name: &str) -> bool {
    matches!(name, "alert" | "confirm" | "prompt")
}

impl JsRule for NoAlert {
    fn meta(&self) -> &'static RuleMeta {
        &NO_ALERT
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CallExpression]
    }

    fn callee_names(&self) -> &'static [&'static str] {
        &["alert", "confirm", "prompt"]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::CallExpression(call) = node.kind() else {
            return;
        };
        let callee = helpers::unwrap_expr(&call.callee);
        let name = match callee {
            Expression::Identifier(id) => {
                if !is_dialog(id.name.as_str()) || !ctx.is_global_reference(id) {
                    return;
                }
                id.name.as_str()
            }
            Expression::StaticMemberExpression(m) => {
                let name = m.property.name.as_str();
                if !is_dialog(name)
                    || !(ctx.is_global(&m.object, "window")
                        || ctx.is_global(&m.object, "self")
                        || ctx.is_global(&m.object, "globalThis"))
                {
                    return;
                }
                name
            }
            _ => return,
        };
        ctx.emit(
            callee.span(),
            format!("Replace this blocking '{name}' dialog with a non-blocking UI"),
        );
    }
}
