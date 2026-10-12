//! `java/synchronized-on-value` — locking on strings and boxed primitives.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["concurrency", "multi-threading"],
    remediation_minutes: 10,
    description: r#"Detects `synchronized` blocks whose lock is a string literal, or a
variable/field of type `String` or of a boxed primitive (`Integer`, `Boolean`, `Long`, …).

**Why it matters.** Such values have no private identity. String literals and constants are
interned and shared across the whole JVM; small boxed values come from shared caches
(`Integer.valueOf`, `Boolean.TRUE`). Unrelated code that locks "its own" `"LOCK"` or `0`
contends with, or deadlocks against, yours. Conversely, two equal strings built at run time
are different objects and don't exclude each other at all.

**How to fix.** Lock on a dedicated private object.

```java
// Noncompliant
private static final String LOCK = "LOCK";
synchronized (LOCK) { … }

// Compliant
private static final Object LOCK = new Object();
synchronized (LOCK) { … }
```

Variables and fields of the JDK's value-based classes (`Optional`, `LocalDate`, `Instant`, `Duration`
and the other `java.time` values) are reported too: equal instances may be the same shared
object, and the JDK warns against locking them.

Only literals, variables, and fields are checked (not method results such as `key.intern()`).
A variable initialized with `new String(…)` / `new Integer(…)` is a unique object and is not
reported."#,
    ..RuleMeta::new(
        "java/synchronized-on-value",
        "java",
        "Synchronizing on a string or boxed value",
    )
};

pub struct SynchronizedOnValue;

impl<'t> JavaRule<'t> for SynchronizedOnValue {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["synchronized_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(lock) = named_children(node)
            .find(|c| kind(*c) == "parenthesized_expression")
            .map(syntax::strip_parens)
        else {
            return;
        };
        match kind(lock) {
            "string_literal" => {
                ctx.emit(
                    lock,
                    "Lock on a dedicated private object instead of a string literal, which is shared by the whole JVM",
                );
                return;
            }
            "identifier" | "field_access" => {}
            _ => return,
        }
        let Some(t) = ctx.type_of(lock) else {
            return;
        };
        let what = if t.is_string() {
            "String"
        } else if t.is_boxed() {
            t.simple_name().unwrap_or("boxed")
        } else if t.fqn().is_some_and(|f| VALUE_BASED.contains(&f)) {
            let name = t.simple_name().unwrap_or("value-based");
            ctx.emit(
                lock,
                format!("Lock on a dedicated private object instead of this `{name}`: value-based objects have no reliable identity (equal values may be the same shared instance)"),
            );
            return;
        } else {
            return;
        };
        if initialized_with_new(lock, ctx) {
            return;
        }
        ctx.emit(
            lock,
            format!("Lock on a dedicated private object instead of this `{what}` value, which may be shared or interned"),
        );
    }
}

/// Whether the variable/field used as lock is initialized with `new …` (a unique object).
fn initialized_with_new<'t>(lock: Node<'t>, ctx: &Ctx<'t>) -> bool {
    let value = if kind(lock) == "identifier" {
        ctx.declaration_of(lock).and_then(|d| d.value)
    } else {
        util::simple_or_this_field(lock).and_then(|(field, _)| {
            let body = ctx.enclosing_class(lock).and_then(syntax::type_body)?;
            let (_, declarator) = util::find_field(ctx, body, ctx.text(field))?;
            declarator.field(Field::Value)
        })
    };
    value.is_some_and(|v| kind(syntax::strip_parens(v)) == "object_creation_expression")
}

/// JDK value-based classes (besides the boxes and `String`): instances are interchangeable
/// when equal, may be cached and shared, and `synchronized` on them is discouraged by the JDK.
const VALUE_BASED: &[&str] = &[
    "java.util.Optional",
    "java.util.OptionalInt",
    "java.util.OptionalLong",
    "java.util.OptionalDouble",
    "java.time.Duration",
    "java.time.Instant",
    "java.time.LocalDate",
    "java.time.LocalDateTime",
    "java.time.LocalTime",
    "java.time.MonthDay",
    "java.time.OffsetDateTime",
    "java.time.OffsetTime",
    "java.time.Period",
    "java.time.Year",
    "java.time.YearMonth",
    "java.time.ZonedDateTime",
    "java.time.ZoneOffset",
];
