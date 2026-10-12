//! `js/no-ex-assign`

use crate::prelude::*;

pub struct NoExAssign;

pub static NO_EX_ASSIGN: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["error-handling"],
    remediation_minutes: 5,
    description: r#"Flags assignments to the parameter of a `catch` clause (`catch (e) { e = … }`).

**Why:** after the assignment, the caught exception is gone: later code in the handler — logging,
rethrowing, cleanup decisions — works with the replacement instead, and the original stack trace
and message are lost. Readers also expect the catch parameter to be the thrown value.

**Fix:** store the derived value in a new variable.

```js
try { run(); } catch (err) {
  err = new Error('run failed');           // flagged: original error lost
  throw err;
}

try { run(); } catch (err) {
  throw new Error('run failed', { cause: err }); // compliant
}
```
"#,
    ..RuleMeta::new(
        "js/no-ex-assign",
        "js",
        "Catch clause parameters should not be reassigned",
    )
};

impl JsRule for NoExAssign {
    fn meta(&self) -> &'static RuleMeta {
        &NO_EX_ASSIGN
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CatchParameter]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::CatchParameter(param) = node.kind() else {
            return;
        };
        let mut writes: Vec<(Span, &'a str)> = Vec::new();
        helpers::for_each_binding(&param.pattern, &mut |id| {
            let Some(symbol) = ctx.binding_symbol(id) else {
                return;
            };
            for reference in ctx.references_of(symbol) {
                if reference.is_write() {
                    writes.push((
                        ctx.node(reference.node_id()).span(),
                        ctx.symbol_name(symbol),
                    ));
                }
            }
        });
        for (span, name) in writes {
            ctx.emit(
                span,
                format!("Do not reassign the caught exception '{name}'; store the new value in another variable"),
            );
        }
    }
}
