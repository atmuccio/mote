//! `java/useless-catch` — a catch block that only rethrows the caught exception.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["error-handling", "redundant"],
    remediation_minutes: 2,
    description: r#"Detects `catch` clauses whose only statement rethrows the caught exception
unchanged: `catch (IOException e) { throw e; }`.

**Why it matters.** Such a handler does nothing the runtime wouldn't do anyway; it only adds
noise and suggests handling that doesn't exist.

**How to fix.** Remove the `catch` clause (and the `try` if nothing else is left), or actually
handle the exception: add context, translate it, clean up.

```java
// Noncompliant
try {
    store.save(order);
} catch (SQLException e) {
    throw e;
}

// Compliant
store.save(order);
```

Only the **last** `catch` clause of a `try` is checked: a rethrowing clause followed by other
clauses keeps its exception away from the later, broader handlers and is therefore
meaningful."#,
    ..RuleMeta::new(
        "java/useless-catch",
        "java",
        "Catch block that only rethrows",
    )
};

pub struct UselessCatch;

impl<'t> JavaRule<'t> for UselessCatch {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["catch_clause"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        let mut stmts = util::statements(body);
        let (Some(only), None) = (stmts.next(), stmts.next()) else {
            return;
        };
        if kind(only) != "throw_statement" {
            return;
        }
        let Some(thrown) = syntax::first_named_child(only).map(syntax::strip_parens) else {
            return;
        };
        let Some(name) = util::catch_param_name(node, ctx) else {
            return;
        };
        if kind(thrown) != "identifier" || ctx.text(thrown) != name {
            return;
        }
        // A later catch clause could otherwise handle this exception: keep the rethrow.
        let mut sib = ctx.next_named_sibling(node);
        while let Some(s) = sib {
            match kind(s) {
                "catch_clause" => return,
                "line_comment" | "block_comment" => sib = ctx.next_named_sibling(s),
                _ => break,
            }
        }
        let at = syntax::children(node).next().unwrap_or(node);
        ctx.emit(
            at,
            format!("Remove this catch clause; it only rethrows `{name}` unchanged"),
        );
    }
}
