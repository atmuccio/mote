//! `java/wait-notify-wrong-monitor` — `lock.wait()` / `lock.notify()` inside synchronized code
//! that holds a different monitor.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["concurrency", "multi-threading"],
    remediation_minutes: 10,
    description: r#"Detects `wait()`, `notify()` and `notifyAll()` called on an object (`lock.wait()`)
from synchronized code that holds some other monitor: inside `synchronized (other) { … }` or a
`synchronized` method (which holds `this`), with no enclosing `synchronized (lock)`.

**Why it matters.** These methods require the caller to own the monitor *of the object they
are called on*. Holding another lock does not count: unless every caller already holds the
right monitor, the call throws `IllegalMonitorStateException`. The code looks right at a glance
because it is inside a `synchronized` block, just not the one for this object.

**How to fix.** Synchronize on the object you wait on or notify (`synchronized (lock) {
lock.wait(); }`), or call `wait()`/`notify()` on the object you already hold.

```java
// Noncompliant
synchronized void awaitReady() throws InterruptedException {
    while (!ready) {
        readyLock.wait();        // holds `this`, not `readyLock`
    }
}

// Compliant
void awaitReady() throws InterruptedException {
    synchronized (readyLock) {
        while (!ready) {
            readyLock.wait();
        }
    }
}
```

The receiver must be a plain name or a field path (`lock`, `this.lock`, `a.lock`); it is
compared textually with the locks of the enclosing `synchronized` blocks. One alias is
recognized: a local variable initialized with the other expression and never reassigned
(`Buffer b = buffer; synchronized (b) { buffer.wait(); }` holds the right monitor); other
aliases are not, and such calls stay silent only when a matching block is present. Calls in
lambdas or code outside any synchronized context, and methods that check
`Thread.holdsLock(…)` (their callers hold the lock), are not reported; `wait`
calls whose arguments are not proven numeric are skipped (they may be overloads)."#,
    ..RuleMeta::new(
        "java/wait-notify-wrong-monitor",
        "java",
        "wait/notify on an object whose monitor is not held",
    )
};

pub struct WaitNotifyWrongMonitor;

impl<'t> JavaRule<'t> for WaitNotifyWrongMonitor {
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
        let Some(method) = ctx.call_name(node) else {
            return;
        };
        let args = syntax::argument_count(node);
        match method {
            "wait" if args <= 2 => {}
            "notify" | "notifyAll" if args == 0 => {}
            _ => return,
        }
        let Some(recv) = node.field(Field::Object) else {
            return;
        };
        let Some(target) = monitor_key(recv, ctx) else {
            return;
        };
        if target == "this" {
            return;
        }
        // Collect the monitors held by the enclosing synchronized code of this method.
        let mut held_any = false;
        for a in ctx.ancestors(node) {
            match kind(a) {
                "synchronized_statement" => {
                    held_any = true;
                    let lock_expr = named_children(a)
                        .find(|c| kind(*c) == "parenthesized_expression")
                        .and_then(|p| syntax::first_named_child(p));
                    let lock = lock_expr.and_then(|l| monitor_key(l, ctx));
                    match lock {
                        Some(l) if l == target => return,
                        // A local copy of the other one: the same object.
                        Some(l)
                            if lock_expr.and_then(|e| local_copy_of(e, ctx)) == Some(target)
                                || local_copy_of(recv, ctx) == Some(l) =>
                        {
                            return
                        }
                        Some(_) => {}
                        // An expression we can't compare might be the same object.
                        None => return,
                    }
                }
                "method_declaration" => {
                    if syntax::has_modifier(a, "synchronized") {
                        held_any = true;
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
                | "static_initializer" => return,
                _ => {}
            }
        }
        if !held_any {
            return;
        }
        // `assert Thread.holdsLock(lock)`: the caller is documented to hold it.
        if let Some(m) = ctx.enclosing_method(node) {
            if ctx.text(m).contains("holdsLock") {
                return;
            }
        }
        // `wait(long)` / `wait(long, int)` only: other `wait` signatures are overloads.
        if args > 0
            && !syntax::arguments(node).all(|a| {
                ctx.type_of(a)
                    .and_then(|t| t.prim())
                    .is_some_and(|p| p.is_integral())
            })
        {
            return;
        }
        let at = node.field(Field::Name).unwrap_or(node);
        ctx.emit(
            at,
            format!("Synchronize on `{target}` before calling `{method}()` on it; the code here holds a different monitor"),
        );
    }
}

/// A comparable key for a monitor expression: `lock`, `this.lock` → `lock`, `a.b`, `this`;
/// `None` for anything else.
fn monitor_key<'t>(expr: Node<'t>, ctx: &Ctx<'t>) -> Option<&'t str> {
    let e = syntax::strip_parens(expr);
    match kind(e) {
        "this" | "identifier" => Some(ctx.text(e)),
        "field_access" => {
            let obj = e.field(Field::Object)?;
            let field = e.field(Field::Field)?;
            if kind(obj) == "this" {
                return Some(ctx.text(field));
            }
            let mut o = obj;
            loop {
                match kind(o) {
                    "identifier" => break,
                    "field_access" => o = o.field(Field::Object)?,
                    _ => return None,
                }
            }
            Some(ctx.text(e))
        }
        _ => None,
    }
}

/// The monitor key of the initializer of a local variable named by `expr`
/// (`ByteBuffer copy = buffer;` → `buffer`): the variable holds the same object, as long as it
/// is not reassigned.
fn local_copy_of<'t>(expr: Node<'t>, ctx: &Ctx<'t>) -> Option<&'t str> {
    let e = syntax::strip_parens(expr);
    if kind(e) != "identifier" {
        return None;
    }
    let decl = ctx.declaration_of(e)?;
    if decl.kind != VarKind::Local {
        return None;
    }
    let key = monitor_key(decl.value?, ctx)?;
    if key == "this" {
        return None;
    }
    // Reassigned somewhere in its block: it may hold another object by now.
    let name = ctx.text(e);
    let scope = ctx.parent(decl.statement)?;
    let reassigned = super::util::any_descendant(scope, |n| {
        kind(n) == "assignment_expression"
            && n.field(Field::Left)
                .map(syntax::strip_parens)
                .is_some_and(|l| kind(l) == "identifier" && ctx.text(l) == name)
    });
    (!reassigned).then_some(key)
}
