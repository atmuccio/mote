//! `java/virtual-thread-platform-setting` — `setDaemon(false)` or `setPriority(..)` on a
//! virtual thread.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["concurrency", "virtual-threads"],
    remediation_minutes: 5,
    description: r#"Detects settings that only platform threads support, applied to a thread
created as a virtual thread (`Thread.ofVirtual()…unstarted(..)` / `.start(..)`,
`Thread.startVirtualThread(..)`):

* `setDaemon(false)` — virtual threads are always daemon threads; the call throws
  `IllegalArgumentException`;
* `setPriority(..)` — virtual threads always run at normal priority; the call is silently
  ignored.

**Why it matters.** Code moved from platform to virtual threads keeps its old tuning: the
daemon setting now crashes at run time, and a priority meant to favour latency-critical
work does nothing.

**How to fix.** Remove the call. Work that must keep the JVM alive or needs a priority
belongs on a platform thread (`Thread.ofPlatform().daemon(false).priority(..)`).

```java
// Noncompliant
Thread worker = Thread.ofVirtual().unstarted(task);
worker.setDaemon(false);
worker.start();

// Compliant
Thread worker = Thread.ofPlatform().daemon(false).unstarted(task);
worker.start();
```

Only direct chains and local variables initialized with a virtual thread and never
reassigned are followed; `setDaemon(true)` (a no-op) is not reported."#,
    ..RuleMeta::new(
        "java/virtual-thread-platform-setting",
        "java",
        "Platform-only setting on a virtual thread",
    )
};

#[derive(Default)]
pub struct VirtualThreadPlatformSetting {
    gate: util::Gate,
}

/// Builder methods that keep a `Thread.Builder.OfVirtual`.
const BUILDER_STEPS: &[&str] = &[
    "name",
    "inheritInheritableThreadLocals",
    "uncaughtExceptionHandler",
];

/// Whether `expr` creates a virtual thread.
fn is_virtual_thread<'t>(ctx: &Ctx<'t>, expr: Node<'t>) -> bool {
    let expr = syntax::strip_parens(expr);
    if kind(expr) != "method_invocation" {
        return false;
    }
    let Some(obj) = expr.field(Field::Object) else {
        return false;
    };
    match ctx.call_name(expr) {
        Some("startVirtualThread") => util::names_type(ctx, obj, "java.lang.Thread"),
        Some("unstarted" | "start") if util::arg_count(expr) == 1 => {
            let mut cur = syntax::strip_parens(obj);
            loop {
                if kind(cur) != "method_invocation" {
                    return false;
                }
                let Some(inner) = cur.field(Field::Object) else {
                    return false;
                };
                match ctx.call_name(cur) {
                    Some("ofVirtual") => {
                        return util::arg_count(cur) == 0
                            && util::names_type(ctx, inner, "java.lang.Thread");
                    }
                    Some(step) if BUILDER_STEPS.contains(&step) => {
                        cur = syntax::strip_parens(inner)
                    }
                    _ => return false,
                }
            }
        }
        _ => false,
    }
}

impl<'t> JavaRule<'t> for VirtualThreadPlatformSetting {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["setDaemon", "setPriority"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) = ctx.call_name(node) else {
            return;
        };
        if !matches!(name, "setDaemon" | "setPriority") || util::arg_count(node) != 1 {
            return;
        }
        let daemon_false = util::arg(node, 0)
            .map(syntax::strip_parens)
            .is_some_and(|a| kind(a) == "false");
        if name == "setDaemon" && !daemon_false {
            return;
        }
        if !self.gate.open(ctx, "Virtual") {
            return;
        }
        let Some(obj) = node.field(Field::Object).map(syntax::strip_parens) else {
            return;
        };
        let virtual_thread = match kind(obj) {
            "method_invocation" => is_virtual_thread(ctx, obj),
            "identifier" => local_virtual_thread(ctx, obj),
            _ => false,
        };
        if !virtual_thread {
            return;
        }
        let msg = if name == "setDaemon" {
            "Remove `setDaemon(false)`: virtual threads are always daemon threads, so this call throws IllegalArgumentException"
        } else {
            "Remove `setPriority(..)`: a virtual thread's priority cannot be changed, so this call has no effect"
        };
        let at = node.field(Field::Name).unwrap_or(node);
        ctx.emit(at, msg);
    }
}

/// Whether `ident` is a local variable initialized with a virtual thread and never assigned
/// again in its method.
fn local_virtual_thread<'t>(ctx: &Ctx<'t>, ident: Node<'t>) -> bool {
    let Some(decl) = ctx.declaration_of(ident) else {
        return false;
    };
    if decl.kind != VarKind::Local {
        return false;
    }
    let Some(value) = decl.value else {
        return false;
    };
    if !is_virtual_thread(ctx, value) {
        return false;
    }
    let Some(body) = ctx
        .enclosing_function(ident)
        .and_then(|f| f.field(Field::Body))
    else {
        return false;
    };
    let name = ctx.text(ident);
    let mut reassigned = false;
    util::visit(
        body,
        |_| true,
        |n| {
            if kind(n) == "assignment_expression"
                && n.field(Field::Left)
                    .is_some_and(|l| kind(l) == "identifier" && ctx.text(l) == name)
            {
                reassigned = true;
            }
        },
    );
    !reassigned
}
