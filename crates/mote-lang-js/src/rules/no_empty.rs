//! `js/no-empty`

use crate::prelude::*;

pub struct NoEmpty;

pub static NO_EMPTY: RuleMeta = RuleMeta {
    severity: Severity::Medium,
    tags: &["suspicious"],
    remediation_minutes: 5,
    description: r#"Flags empty block statements: empty `if`/`else` branches, loop bodies, `try`, `catch`,
and `finally` blocks, stand-alone `{}` blocks, and `switch` statements without cases.

**Why:** an empty block usually means unfinished code, or an error that is silently swallowed (an
empty `catch`). When doing nothing *is* intended, readers cannot tell without a note.

**Fix:** add the missing code, remove the block, or put a comment inside explaining why nothing needs
to happen. A block containing a comment is not flagged. Empty function bodies are not covered by
this rule.

```js
try {
  save();
} catch (e) {}            // flagged

try {
  save();
} catch (e) {
  // Best effort: the cache is rebuilt on next start.
}                         // compliant
```
"#,
    ..RuleMeta::new("js/no-empty", "js", "Blocks should not be empty")
};

impl JsRule for NoEmpty {
    fn meta(&self) -> &'static RuleMeta {
        &NO_EMPTY
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::BlockStatement, AstType::SwitchStatement]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        match node.kind() {
            // Function bodies are `FunctionBody` nodes, so they never reach this arm.
            AstKind::BlockStatement(block) => {
                if block.body.is_empty() && !ctx.has_comments_in(block.span) {
                    let message = match ctx.parent_kind(node) {
                        Some(AstKind::CatchClause(_)) => {
                            "Handle this exception or add a comment explaining why it is ignored"
                        }
                        _ => "Add code or an explanatory comment to this empty block, or remove it",
                    };
                    ctx.emit(block.span, message);
                }
            }
            AstKind::SwitchStatement(switch)
                if switch.cases.is_empty() && !ctx.has_comments_in(switch.span) =>
            {
                let keyword = Span::new(switch.span.start, switch.span.start + 6);
                ctx.emit(keyword, "Add cases to this empty 'switch' or remove it");
            }
            _ => {}
        }
    }
}
