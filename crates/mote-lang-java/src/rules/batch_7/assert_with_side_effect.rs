//! `java/assert-with-side-effect` — `assert` conditions that change state.

use super::util::{self, Walk};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["assertions", "side-effects"],
    remediation_minutes: 5,
    description: r#"Detects `assert` statements whose condition changes state: an assignment, an
increment or decrement, or a call to a well-known mutating method of a JDK collection,
iterator, atomic variable, lock or file (`list.remove(x)`, `it.next()`,
`counter.incrementAndGet()`, `file.delete()`, …).

**Why it matters.** Assertions are disabled by default at run time (they need `-ea`). With
assertions off, the whole condition is skipped — including its side effect. The program then
behaves differently in production than in tests: the element is never removed, the counter
never incremented, the file never deleted.

**How to fix.** Perform the action unconditionally and assert on its result.

```java
// Noncompliant
assert pending.remove(job) : "job was not pending";

// Compliant
boolean removed = pending.remove(job);
assert removed : "job was not pending";
```

The message part of the `assert` (evaluated only on failure) is not checked. Method calls
count only when the receiver's type is provably one of those JDK types; lambdas in the
condition are skipped."#,
    ..RuleMeta::new(
        "java/assert-with-side-effect",
        "java",
        "Side effect inside an assert",
    )
};

pub struct AssertWithSideEffect;

/// (owner type, mutating methods).
const MUTATORS: &[(&str, &[&str])] = &[
    (
        "java.util.Collection",
        &[
            "add",
            "addAll",
            "remove",
            "removeAll",
            "removeIf",
            "retainAll",
            "clear",
            "offer",
            "poll",
            "push",
            "pop",
        ],
    ),
    (
        "java.util.Map",
        &[
            "put",
            "putAll",
            "putIfAbsent",
            "remove",
            "clear",
            "compute",
            "computeIfAbsent",
            "computeIfPresent",
            "merge",
            "replace",
        ],
    ),
    ("java.util.Iterator", &["next", "remove"]),
    (
        "java.util.concurrent.atomic.AtomicInteger",
        &[
            "incrementAndGet",
            "decrementAndGet",
            "getAndIncrement",
            "getAndDecrement",
            "addAndGet",
            "getAndAdd",
            "getAndSet",
            "compareAndSet",
            "set",
        ],
    ),
    (
        "java.util.concurrent.atomic.AtomicLong",
        &[
            "incrementAndGet",
            "decrementAndGet",
            "getAndIncrement",
            "getAndDecrement",
            "addAndGet",
            "getAndAdd",
            "getAndSet",
            "compareAndSet",
            "set",
        ],
    ),
    (
        "java.util.concurrent.atomic.AtomicBoolean",
        &["getAndSet", "compareAndSet", "set"],
    ),
    (
        "java.util.concurrent.atomic.AtomicReference",
        &["getAndSet", "compareAndSet", "set"],
    ),
    ("java.util.concurrent.locks.Lock", &["tryLock"]),
    ("java.util.concurrent.Semaphore", &["tryAcquire"]),
    (
        "java.io.File",
        &["delete", "mkdir", "mkdirs", "createNewFile", "renameTo"],
    ),
];

fn is_mutator_name(name: &str) -> bool {
    MUTATORS.iter().any(|(_, ms)| ms.contains(&name))
}

impl<'t> JavaRule<'t> for AssertWithSideEffect {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["assert_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(cond) = syntax::first_named_child(node) else {
            return;
        };
        let mut hit: Option<(Node<'t>, String)> = None;
        let mut look = |n: Node<'t>| -> Walk {
            if util::is_nested_scope(n) {
                return Walk::Skip;
            }
            match kind(n) {
                "assignment_expression" => {
                    hit = Some((n, "an assignment".to_string()));
                    Walk::Stop
                }
                "update_expression" => {
                    hit = Some((n, format!("`{}`", ctx.text(n))));
                    Walk::Stop
                }
                "method_invocation" => {
                    let Some(name) = ctx.call_name(n) else {
                        return Walk::Continue;
                    };
                    if !is_mutator_name(name) {
                        return Walk::Continue;
                    }
                    let Some(obj) = n.field(Field::Object) else {
                        return Walk::Continue;
                    };
                    if matches!(kind(obj), "this" | "super") {
                        return Walk::Continue;
                    }
                    let Some(t) = ctx.type_of(obj) else {
                        return Walk::Continue;
                    };
                    let mutates = MUTATORS
                        .iter()
                        .any(|(owner, ms)| ms.contains(&name) && ctx.is_subtype_of(&t, owner));
                    if mutates {
                        hit = Some((n, format!("`{name}(..)`")));
                        Walk::Stop
                    } else {
                        Walk::Continue
                    }
                }
                _ => Walk::Continue,
            }
        };
        if look(cond) != Walk::Stop {
            util::visit(cond, &mut look);
        }
        if let Some((at, what)) = hit {
            ctx.emit(
                at,
                format!("Move {what} out of the assert: with assertions disabled (the default) it never runs"),
            );
        }
    }
}
