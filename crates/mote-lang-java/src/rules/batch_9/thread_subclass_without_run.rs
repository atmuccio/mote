//! `java/thread-subclass-without-run` — a `Thread` subclass that neither overrides `run()` nor
//! receives a `Runnable`.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["concurrency"],
    remediation_minutes: 10,
    description: r#"Detects a class extending `java.lang.Thread` directly — named or anonymous —
that does not override `run()` and never passes a `Runnable` to the `Thread` constructor.

**Why it matters.** `Thread.run()` only runs the `Runnable` given at construction. Without
one and without an override, starting the thread does nothing: the work the class was meant
to do in the background silently never happens. The usual causes are a misspelled or
mis-declared `run` (`Run()`, `run(String)`), or work placed in the constructor.

**How to fix.** Override `public void run()` with the thread's work, or better, implement
`Runnable` and hand it to a `Thread` or an executor.

```java
// Noncompliant
class Poller extends Thread {
    public void Run() { poll(); }
}

// Compliant
class Poller extends Thread {
    @Override
    public void run() { poll(); }
}
```

To stay certain that no subclass provides `run()`, a named class is only reported when it is
`final`, or when it is `private`, local to a method, or instantiated directly in the same
file, and no class of the file (named or anonymous) extends it. Abstract classes and classes
overriding `start()` are skipped, as are constructors passing anything but a name or a
thread group to `super(..)`."#,
    ..RuleMeta::new(
        "java/thread-subclass-without-run",
        "java",
        "Thread subclass without run()",
    )
};

/// A named `Thread` subclass with neither `run()` nor a `Runnable`.
struct Candidate<'t> {
    name: &'t str,
    name_node: Node<'t>,
    /// `final`: nothing can extend it.
    final_class: bool,
    /// `private` or local: only this file can extend or create it.
    hidden: bool,
}

#[derive(Default)]
pub struct ThreadSubclassWithoutRun<'t> {
    gate: util::Gate,
    candidates: Vec<Candidate<'t>>,
    /// Simple names of the superclasses of this file's classes (named and anonymous).
    extended: Vec<&'t str>,
    /// Simple names of the classes instantiated directly (`new X(..)` without a body).
    created: Vec<&'t str>,
}

impl<'t> JavaRule<'t> for ThreadSubclassWithoutRun<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["class_declaration", "object_creation_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !self.gate.open(ctx, "Thread") {
            return;
        }
        if kind(node) == "class_declaration" {
            let Some(ty) = node
                .field(Field::Superclass)
                .and_then(syntax::first_named_child)
            else {
                return;
            };
            self.extended.push(simple_name(ctx, ty));
            if let Some(c) = candidate(node, ty, ctx) {
                self.candidates.push(c);
            }
            return;
        }
        let Some(ty) = node.field(Field::Type) else {
            return;
        };
        let name = simple_name(ctx, ty);
        if syntax::child_of_kind(node, "class_body").is_some() {
            self.extended.push(name);
            check_anonymous(node, ctx);
        } else {
            self.created.push(name);
        }
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        for c in &self.candidates {
            let extended_here = self.extended.contains(&c.name);
            let created_here = self.created.contains(&c.name);
            // A subclass (here or, unless hidden, elsewhere) may provide run(), except for
            // instances of the class itself created here.
            let certain = c.final_class || (!extended_here && (c.hidden || created_here));
            if !certain {
                continue;
            }
            ctx.emit(
                c.name_node,
                format!("Override `run()` in `{}` or pass a `Runnable` to the `Thread` constructor: as written, starting this thread does nothing", c.name),
            );
        }
    }
}

/// The simple name of a type node (`Outer.Inner<T>` → `Inner`).
fn simple_name<'t>(ctx: &Ctx<'t>, ty: Node<'t>) -> &'t str {
    let base = if kind(ty) == "generic_type" {
        syntax::first_named_child(ty).unwrap_or(ty)
    } else {
        ty
    };
    util::last_segment(ctx.text(base))
}
/// Whether a type node names `java.lang.Thread` itself.
fn is_thread<'t>(ctx: &Ctx<'t>, ty: Node<'t>) -> bool {
    matches!(ctx.text(ty), "Thread" | "java.lang.Thread")
        && ctx
            .resolve_type(ty)
            .is_some_and(|t| t.is_class("java.lang.Thread"))
}

/// Whether a class body declares a zero-parameter method with one of `names`.
fn declares<'t>(ctx: &Ctx<'t>, body: Node<'t>, names: &[&str]) -> bool {
    ctx.members(body).any(|m| {
        kind(m) == "method_declaration"
            && names.contains(&syntax::name_of(m, ctx.source()))
            && syntax::parameters(m).next().is_none()
    })
}

/// Whether every argument is provably a name (`String`) or a `ThreadGroup`.
fn only_names<'t>(ctx: &Ctx<'t>, call: Node<'t>) -> bool {
    util::args(call).all(|a| {
        kind(syntax::strip_parens(a)) == "string_literal"
            || ctx
                .type_of(a)
                .is_some_and(|t| t.is_string() || t.is_class("java.lang.ThreadGroup"))
    })
}

fn candidate<'t>(decl: Node<'t>, ty: Node<'t>, ctx: &Ctx<'t>) -> Option<Candidate<'t>> {
    if !is_thread(ctx, ty) || syntax::has_modifier(decl, "abstract") {
        return None;
    }
    let body = decl.field(Field::Body)?;
    if declares(ctx, body, &["run", "start"]) {
        return None;
    }
    // Constructors may hand a Runnable to Thread: only names and thread groups are fine.
    for m in ctx.members(body) {
        if kind(m) != "constructor_declaration" {
            continue;
        }
        let cbody = m.field(Field::Body)?;
        // Since Java 22 (flexible constructor bodies) statements may precede `super(...)`.
        let Some(call) =
            named_children(cbody).find(|c| kind(*c) == "explicit_constructor_invocation")
        else {
            continue;
        };
        if call
            .field(Field::Constructor)
            .is_some_and(|c| kind(c) == "super")
            && !only_names(ctx, call)
        {
            return None;
        }
    }
    Some(Candidate {
        name: syntax::name_of(decl, ctx.source()),
        name_node: decl.field(Field::Name)?,
        final_class: syntax::has_modifier(decl, "final"),
        hidden: syntax::has_modifier(decl, "private")
            || ctx.parent(decl).is_some_and(|p| kind(p) == "block"),
    })
}

fn check_anonymous<'t>(node: Node<'t>, ctx: &Ctx<'t>) {
    let Some(ty) = node.field(Field::Type) else {
        return;
    };
    if !matches!(ctx.text(ty), "Thread" | "java.lang.Thread") {
        return;
    }
    let Some(body) = syntax::child_of_kind(node, "class_body") else {
        return;
    };
    if !is_thread(ctx, ty) || !only_names(ctx, node) || declares(ctx, body, &["run", "start"]) {
        return;
    }
    ctx.emit(
        ty,
        "Override `run()` in this anonymous `Thread` or pass it a `Runnable`: as written, starting it does nothing",
    );
}
