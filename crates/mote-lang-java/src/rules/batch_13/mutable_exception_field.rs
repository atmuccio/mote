//! `java/mutable-exception-field` — non-final instance fields in an exception class.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["exceptions", "immutability"],
    remediation_minutes: 5,
    description: r#"Detects instance fields that are not `final` in a class whose superclass chain
provably reaches `Throwable` (through classes of this file or JDK exceptions).

**Why it matters.** An exception is a report of something that happened: it is created at one
place, travels up the stack, is often handed to other threads (futures, executors, log
appenders) and may be serialized. A handler that changes it — or a field set later through a
setter — changes the report for every other holder, and a field written without
synchronization may not even be visible to the thread that reads it. Final fields make the
exception a safe, self-describing value.

**How to fix.** Declare the fields `final` and set them in the constructor; if a value is only
known later, create a new exception (with the original as its cause) instead of mutating it.

```java
// Noncompliant
class OrderFailedException extends RuntimeException {
    private String orderId;
    void setOrderId(String id) { this.orderId = id; }
}

// Compliant
class OrderFailedException extends RuntimeException {
    private final String orderId;
    OrderFailedException(String orderId, Throwable cause) {
        super(cause);
        this.orderId = orderId;
    }
}
```

Not reported: `static` fields, classes whose superclass is declared in another file (whether
they are exceptions is not visible here), classes using Lombok (it may generate final
fields and constructors), and classes with a custom `readObject` (deserialization assigns
their fields after construction)."#,
    ..RuleMeta::new(
        "java/mutable-exception-field",
        "java",
        "Mutable field in an exception class",
    )
};

pub struct MutableExceptionField;

impl<'t> JavaRule<'t> for MutableExceptionField {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["class_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if node.field(Field::Superclass).is_none() {
            return;
        }
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        // Cheap: any non-final, non-static field at all?
        let mut candidates = ctx
            .members(body)
            .filter(|m| {
                kind(*m) == "field_declaration"
                    && !syntax::has_modifier(*m, "final")
                    && !syntax::has_modifier(*m, "static")
            })
            .peekable();
        if candidates.peek().is_none() {
            return;
        }
        let Some(t) = ctx.class_type(node) else {
            return;
        };
        if !ctx.is_subtype_of(&t, "java.lang.Throwable") || ctx.has_lombok(node) {
            return;
        }
        // Custom deserialization assigns fields after construction.
        let src = ctx.source();
        if ctx
            .members(body)
            .any(|m| kind(m) == "method_declaration" && syntax::name_of(m, src) == "readObject")
        {
            return;
        }
        let class_name = syntax::name_of(node, ctx.source());
        for field in candidates {
            for d in syntax::declarators(field) {
                let Some(name) = d.field(Field::Name) else {
                    continue;
                };
                ctx.emit(
                    name,
                    format!(
                        "Make `{}` final: the exception `{class_name}` should not change after it is created",
                        ctx.text(name)
                    ),
                );
            }
        }
    }
}
