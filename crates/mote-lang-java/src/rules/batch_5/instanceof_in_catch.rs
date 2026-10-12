//! `java/instanceof-in-catch` — `catch (Exception e) { if (e instanceof IOException) … }`.

use super::util::{self, Walk, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["exceptions", "readability"],
    remediation_minutes: 5,
    description: r#"Detects `instanceof` tests on the caught exception inside a `catch` block.

**Why it matters.** Sorting exceptions by type is exactly what separate `catch` clauses do.
Doing it by hand with `instanceof` (and usually a cast) is longer, easier to get wrong
(order, a missing `else`), and hides from the reader which exceptions get special treatment.

**How to fix.** Add a dedicated `catch` clause for each type that needs different handling,
before the broader one.

```java
// Noncompliant
try {
    sync();
} catch (Exception e) {
    if (e instanceof TimeoutException) {
        retryLater();
    } else {
        fail(e);
    }
}

// Compliant
try {
    sync();
} catch (TimeoutException e) {
    retryLater();
} catch (Exception e) {
    fail(e);
}
```

Only tests of the exception variable itself are reported (not of its cause, as in
`e.getCause() instanceof …`)."#,
    ..RuleMeta::new(
        "java/instanceof-in-catch",
        "java",
        "instanceof check on a caught exception",
    )
};

pub struct InstanceofInCatch;

impl<'t> JavaRule<'t> for InstanceofInCatch {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["catch_clause"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) = named_children(node)
            .find(|c| kind(*c) == "catch_formal_parameter")
            .and_then(|p| util::fld(p, F.name))
        else {
            return;
        };
        if kind(name) != "identifier" {
            return;
        }
        let var = ctx.text(name);
        let Some(body) = util::fld(node, F.body) else {
            return;
        };
        if !ctx.text(body).contains("instanceof") {
            return;
        }
        let mut hits: Vec<Node<'t>> = Vec::new();
        // The first reassignment of the variable (`ex = ((Wrapper) ex).getCause();`): after it,
        // a test no longer looks at the caught exception.
        let mut reassigned_at = usize::MAX;
        util::visit(body, |n| {
            if kind(n) == "class_body" {
                return Walk::Skip;
            }
            if kind(n) == "assignment_expression"
                && util::fld(n, F.left)
                    .is_some_and(|l| kind(l) == "identifier" && ctx.text(l) == var)
            {
                reassigned_at = reassigned_at.min(n.start_byte());
            }
            if kind(n) == "instanceof_expression" {
                if let Some(left) = util::fld(n, F.left).map(syntax::strip_parens) {
                    if kind(left) == "identifier" && ctx.text(left) == var {
                        hits.push(n);
                    }
                }
            }
            Walk::Continue
        });
        for n in hits.into_iter().filter(|n| n.start_byte() < reassigned_at) {
            ctx.emit(
                n,
                format!(
                    "Catch this exception type in its own clause instead of testing `{var}` with \
                     `instanceof`"
                ),
            );
        }
    }
}
