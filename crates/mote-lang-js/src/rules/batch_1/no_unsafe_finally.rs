//! `js/no-unsafe-finally`

use crate::prelude::*;

pub struct NoUnsafeFinally;

pub static NO_UNSAFE_FINALLY: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["error-handling"],
    remediation_minutes: 10,
    description: r#"Flags `return`, `throw`, `break`, and `continue` statements that leave a `finally` block.

**Why:** a `finally` block runs after the `try` or `catch` has already decided how to finish —
returning a value or propagating an exception. Leaving `finally` with a jump throws that decision
away: an exception from the `try` is silently swallowed, or the returned value is replaced. The
code then behaves as if the error never happened.

**Fix:** keep `finally` to cleanup only; move the `return`/`throw` into the `try` or `catch`.

```js
function read() {
  try {
    return parse();
  } finally {
    return null;          // flagged: discards parse()'s result and any exception
  }
}

function cleanup(items) {
  try {
    process(items);
  } finally {
    for (const item of items) {
      if (item.done) break; // compliant: the break stays inside the finally block
      item.close();
    }
  }
}
```

Jumps that stay inside the `finally` block (a `break` of a loop inside it, a `throw` caught by a
`try` inside it) and code in nested functions are not reported.
"#,
    ..RuleMeta::new(
        "js/no-unsafe-finally",
        "js",
        "Control flow should not leave a 'finally' block",
    )
};

impl JsRule for NoUnsafeFinally {
    fn meta(&self) -> &'static RuleMeta {
        &NO_UNSAFE_FINALLY
    }

    fn types(&self) -> &'static [AstType] {
        &[
            AstType::ReturnStatement,
            AstType::ThrowStatement,
            AstType::BreakStatement,
            AstType::ContinueStatement,
        ]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.file_contains(AstType::TryStatement)
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        #[derive(Clone, Copy, PartialEq)]
        enum Jump {
            Return,
            Throw,
            Break,
            Continue,
        }
        let (jump, label, keyword) = match node.kind() {
            AstKind::ReturnStatement(_) => (Jump::Return, None, "return"),
            AstKind::ThrowStatement(_) => (Jump::Throw, None, "throw"),
            AstKind::BreakStatement(b) => (Jump::Break, b.label.as_ref(), "break"),
            AstKind::ContinueStatement(c) => (Jump::Continue, c.label.as_ref(), "continue"),
            _ => return,
        };
        let mut child = node.span();
        for ancestor in ctx.ancestors(node) {
            match ancestor.kind() {
                AstKind::Function(_)
                | AstKind::ArrowFunctionExpression(_)
                | AstKind::StaticBlock(_)
                | AstKind::PropertyDefinition(_)
                | AstKind::AccessorProperty(_)
                | AstKind::Class(_)
                | AstKind::Program(_) => return,
                AstKind::LabeledStatement(l) => {
                    if label.is_some_and(|name| name.name == l.label.name) {
                        return;
                    }
                }
                AstKind::SwitchStatement(_) => {
                    if jump == Jump::Break && label.is_none() {
                        return;
                    }
                }
                kind if kind.is_iteration_statement() => {
                    if matches!(jump, Jump::Break | Jump::Continue) && label.is_none() {
                        return;
                    }
                }
                AstKind::TryStatement(t) => {
                    if t.finalizer.as_ref().is_some_and(|f| f.span == child) {
                        let span =
                            Span::new(node.span().start, node.span().start + keyword.len() as u32);
                        ctx.emit(
                            span,
                            format!("Remove this '{keyword}': leaving a 'finally' block discards the result or exception of the 'try'"),
                        );
                        return;
                    }
                    if jump == Jump::Throw && t.block.span == child && t.handler.is_some() {
                        return;
                    }
                }
                _ => {}
            }
            child = ancestor.span();
        }
    }
}
