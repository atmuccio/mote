//! `java/synchronized-on-get-class` — `synchronized (getClass())` locks a different monitor in
//! each subclass.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["concurrency", "multi-threading"],
    remediation_minutes: 5,
    description: r#"Detects `synchronized (getClass())` (or `this.getClass()`) in a class that can
be subclassed.

**Why it matters.** `getClass()` returns the runtime class of the object. In a subclass
instance it is the subclass, so code that means to guard state shared by the whole class
hierarchy (usually static state of the declaring class) locks a different monitor for every
subclass, and instances of different subclasses run the "exclusive" section concurrently.

**How to fix.** Lock the declaring class literally, or a dedicated static lock.

```java
// Noncompliant
synchronized (getClass()) {
    instances++;
}

// Compliant
synchronized (Registry.class) {
    instances++;
}
```

Final classes, enums, records and anonymous classes (which cannot be subclassed) are not
reported."#,
    ..RuleMeta::new(
        "java/synchronized-on-get-class",
        "java",
        "Synchronizing on getClass()",
    )
};

pub struct SynchronizedOnGetClass;

impl<'t> JavaRule<'t> for SynchronizedOnGetClass {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["synchronized_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(paren) = syntax::child_of_kind(node, "parenthesized_expression") else {
            return;
        };
        let lock = syntax::strip_parens(paren);
        if ctx.call_name(lock) != Some("getClass") || syntax::argument_count(lock) != 0 {
            return;
        }
        if lock.field(Field::Object).is_some_and(|o| kind(o) != "this") {
            return;
        }
        let Some(class) = ctx.enclosing_class(node) else {
            return;
        };
        if kind(class) != "class_declaration" || syntax::has_modifier(class, "final") {
            return;
        }
        let name = syntax::name_of(class, ctx.source());
        ctx.emit(
            lock,
            format!("Synchronize on `{name}.class` or a dedicated lock: `getClass()` is a different monitor in each subclass"),
        );
    }
}
