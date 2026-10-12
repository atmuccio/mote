//! `java/finalize-override` — declaring `finalize()`.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["resources", "deprecated"],
    remediation_minutes: 20,
    description: r#"Detects classes that declare a `finalize()` method.

**Why it matters.** Finalization is deprecated for removal. There is no guarantee when, or
whether, a finalizer runs; objects with finalizers slow down garbage collection, can be
resurrected, and run their cleanup on an unspecified thread. Resources released only in
`finalize()` leak under load.

**How to fix.** Make the class `AutoCloseable` and release resources in `close()` (used with
try-with-resources); for a safety net, register a `java.lang.ref.Cleaner` action.

```java
// Noncompliant
@Override
protected void finalize() throws Throwable {
    channel.close();
}

// Compliant
@Override
public void close() throws IOException {
    channel.close();
}
```

An empty `final` `finalize()` — the defensive idiom that stops subclasses from adding a
finalizer — is not reported, nor a `final` one whose only statement is `super.finalize();`
(the same idiom, delegating to the inherited no-op)."#,
    ..RuleMeta::new("java/finalize-override", "java", "finalize() declared")
};

pub struct FinalizeOverride;

/// `super.finalize();`
fn is_super_finalize<'t>(ctx: &Ctx<'t>, stmt: Node<'t>) -> bool {
    kind(stmt) == "expression_statement"
        && syntax::first_named_child(stmt).is_some_and(|call| {
            kind(call) == "method_invocation"
                && ctx.call_name(call) == Some("finalize")
                && syntax::argument_count(call) == 0
                && call
                    .field(Field::Object)
                    .is_some_and(|o| kind(o) == "super")
        })
}

impl<'t> JavaRule<'t> for FinalizeOverride {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_declaration"]
    }

    fn method_names(&self) -> &'static [&'static str] {
        &["finalize"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) = util::name_field(node) else {
            return;
        };
        if ctx.text(name) != "finalize" || syntax::parameters(node).next().is_some() {
            return;
        }
        let empty_final = syntax::has_modifier(node, "final")
            && node.field(Field::Body).is_some_and(|b| {
                let mut stmts = named_children(b);
                match (stmts.next(), stmts.next()) {
                    (None, _) => true,
                    (Some(only), None) => is_super_finalize(ctx, only),
                    _ => false,
                }
            });
        if empty_final {
            return;
        }
        ctx.emit(
            name,
            "Release resources with AutoCloseable/try-with-resources or a Cleaner instead of finalize()",
        );
    }
}
