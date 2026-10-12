//! `java/wait-with-nested-lock` — `wait()` while another monitor is also held.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["concurrency", "multi-threading", "deadlock"],
    remediation_minutes: 20,
    description: r#"Detects `wait()` called inside nested `synchronized` regions that hold two or
more different monitors, e.g. `synchronized (a) { synchronized (b) { b.wait(); } }`, when the
same file notifies that monitor (`b.notify()`) while holding one of the other monitors (`a`).

**Why it matters.** `wait()` releases only the monitor of the object it is called on. Every
other lock stays held for the whole wait. When the thread that calls `notify()` needs one of
those outer locks first, it blocks, and the waiter is never woken: a deadlock that
appears only under a particular interleaving, often in production.

**How to fix.** Wait while holding only the monitor being waited on: restructure so the outer
lock is released before waiting, or use a single lock (or a `java.util.concurrent` lock with
several `Condition`s).

```java
// Noncompliant
synchronized (registry) {
    synchronized (queue) {
        while (queue.isEmpty()) {
            queue.wait();          // `registry` stays locked while waiting
        }
    }
}

// Compliant
synchronized (queue) {
    while (queue.isEmpty()) {
        queue.wait();
    }
}
```

Monitors are compared by their text (`lock` and `this.lock` are the same; a `synchronized`
method holds `this`), so the same object locked twice (reentrantly) is not counted twice. Only
regions of the same method are considered: locks held by callers are not visible. Without a
notifier in the file that needs the outer lock, keeping it while waiting can be deliberate
(holding off new work while in-flight work drains) and is not reported."#,
    ..RuleMeta::new(
        "java/wait-with-nested-lock",
        "java",
        "wait() while holding another lock",
    )
};

#[derive(Default)]
pub struct WaitWithNestedLock<'t> {
    /// (the `wait` name node, monitor waited on, other monitors held)
    waits: Vec<(Node<'t>, &'t str, Vec<&'t str>)>,
    /// (notify call name node, monitor notified, monitors held while notifying)
    notifies: Vec<(Node<'t>, &'t str, Vec<&'t str>)>,
}

/// A comparable key for a monitor expression (`this.x` → `x`), or its plain text.
fn key<'t>(expr: Node<'t>, ctx: &Ctx<'t>) -> &'t str {
    let e = syntax::strip_parens(expr);
    if kind(e) == "field_access" && e.field(Field::Object).is_some_and(|o| kind(o) == "this") {
        if let Some(f) = e.field(Field::Field) {
            return ctx.text(f);
        }
    }
    ctx.text(e)
}

/// The monitors held by the enclosing synchronized regions of the same method; `None` when a
/// lock expression can't be read.
fn held_monitors<'t>(node: Node<'t>, ctx: &Ctx<'t>) -> Option<Vec<&'t str>> {
    let mut held: Vec<&'t str> = Vec::new();
    for a in ctx.ancestors(node) {
        match kind(a) {
            "synchronized_statement" => {
                let lock = named_children(a)
                    .find(|c| kind(*c) == "parenthesized_expression")
                    .and_then(|p| syntax::first_named_child(p))?;
                let k = key(lock, ctx);
                if !held.contains(&k) {
                    held.push(k);
                }
            }
            "method_declaration" => {
                if syntax::has_modifier(a, "synchronized") && !held.contains(&"this") {
                    held.push("this");
                }
                break;
            }
            "lambda_expression"
            | "class_body"
            | "interface_body"
            | "enum_body"
            | "enum_body_declarations"
            | "constructor_declaration"
            | "compact_constructor_declaration"
            | "static_initializer" => break,
            _ => {}
        }
    }
    Some(held)
}

impl<'t> JavaRule<'t> for WaitWithNestedLock<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["wait", "notify", "notifyAll"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) = ctx.call_name(node) else {
            return;
        };
        let args = syntax::argument_count(node);
        match name {
            "wait" if args <= 2 => {}
            "notify" | "notifyAll" if args == 0 => {}
            _ => return,
        }
        let target = match node.field(Field::Object) {
            None => "this",
            Some(o) if kind(o) == "super" => return,
            Some(o) => key(o, ctx),
        };
        let Some(held) = held_monitors(node, ctx) else {
            return;
        };
        if !held.contains(&target) {
            return;
        }
        let at = node.field(Field::Name).unwrap_or(node);
        if name != "wait" {
            self.notifies.push((at, target, held));
            return;
        }
        // Only the monitor being waited on is released; any other one stays held.
        if held.len() < 2 {
            return;
        }
        // `wait(long)` / `wait(long, int)` only: other signatures are overloads.
        if args > 0
            && !syntax::arguments(node).all(|a| {
                ctx.type_of(a)
                    .and_then(|t| t.prim())
                    .is_some_and(|p| p.is_integral())
            })
        {
            return;
        }
        let others = held.into_iter().filter(|h| *h != target).collect();
        self.waits.push((at, target, others));
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        for (at, target, others) in &self.waits {
            // A notifier of this file that needs one of the locks the waiter keeps.
            let blocked = self.notifies.iter().find_map(|(n, t, held)| {
                (t == target)
                    .then(|| others.iter().find(|o| held.contains(o)))
                    .flatten()
                    .map(|o| (*n, *o))
            });
            let Some((notifier, outer)) = blocked else {
                continue;
            };
            ctx.emit(
                *at,
                format!("Release `{outer}` before waiting; wait() only releases `{target}`, and the notifier needs `{outer}` too"),
            )
            .secondary(notifier, "notified while holding the same outer lock");
        }
    }
}
