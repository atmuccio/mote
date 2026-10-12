//! `java/redundant-close-in-try-with-resources` — `close()` called as the last statement of a
//! try-with-resources body on one of its own resources.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Incubating,
    tags: &["resources", "redundant"],
    remediation_minutes: 1,
    description: r#"Detects an explicit `close()` on a resource of a try-with-resources statement
when that call is the last statement of the statement's body.

**Why it matters.** The try-with-resources statement closes each resource as soon as its
body ends, in reverse order, and also when an exception is thrown. The explicit call adds
nothing: the resource is closed twice in a row, which is only harmless when the class makes
`close()` idempotent (`AutoCloseable` does not require it), and it suggests the author did not
trust — or did not notice — the automatic close.

**How to fix.** Remove the call.

```java
// Noncompliant
try (BufferedWriter out = Files.newBufferedWriter(path)) {
    out.write(report);
    out.close();
}

// Compliant
try (BufferedWriter out = Files.newBufferedWriter(path)) {
    out.write(report);
}
```

Not reported: a `close()` followed by other statements of the body — closing early, for
instance to flush a stream before reading what it wrote, has an effect — and bodies that
exercise `close()` itself: a body made only of the call, two calls in a row, or a test method
whose name is about closing (tests of idempotent or delegated closing)."#,
    ..RuleMeta::new(
        "java/redundant-close-in-try-with-resources",
        "java",
        "Redundant close() of a try-with-resources resource",
    )
};

pub struct RedundantCloseInTryWithResources;

impl<'t> JavaRule<'t> for RedundantCloseInTryWithResources {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["try_with_resources_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        let mut stmts = util::statements(body);
        let mut prev = None;
        let mut last = None;
        let mut count = 0usize;
        for s in stmts.by_ref() {
            prev = last;
            last = Some(s);
            count += 1;
        }
        let Some(last) = last else {
            return;
        };
        let Some(recv_text) = close_receiver(ctx, last) else {
            return;
        };
        // A body that only closes, or closes twice in a row, is exercising `close()` itself
        // (typically a test of idempotence); so is a test method about closing.
        if count == 1 || prev.and_then(|p| close_receiver(ctx, p)) == Some(recv_text) {
            return;
        }
        if ctx.is_test()
            && ctx.enclosing_method(node).is_some_and(|m| {
                syntax::name_of(m, ctx.source())
                    .to_ascii_lowercase()
                    .contains("close")
            })
        {
            return;
        }
        let Some(call) = syntax::first_named_child(last) else {
            return;
        };
        let Some(resources) = node.field(Field::Resources) else {
            return;
        };
        let is_resource = named_children(resources).any(|r| {
            if kind(r) != "resource" {
                return false;
            }
            match r.field(Field::Name) {
                Some(n) => ctx.text(n) == recv_text,
                // Java 9: `try (existing)` / `try (this.field)`.
                None => syntax::first_named_child(r).is_some_and(|e| ctx.text(e) == recv_text),
            }
        });
        if !is_resource {
            return;
        }
        let Some(name) = call.field(Field::Name) else {
            return;
        };
        ctx.emit(
            name,
            format!("Remove this `close()`: the try-with-resources statement already closes `{recv_text}` here"),
        );
    }
}

/// The receiver text of a statement `x.close();`.
fn close_receiver<'t>(ctx: &Ctx<'t>, stmt: Node<'t>) -> Option<&'t str> {
    if kind(stmt) != "expression_statement" {
        return None;
    }
    let call = syntax::first_named_child(stmt)?;
    if kind(call) != "method_invocation"
        || ctx.call_name(call) != Some("close")
        || syntax::argument_count(call) != 0
    {
        return None;
    }
    Some(ctx.text(call.field(Field::Object)?))
}
