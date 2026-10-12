//! `java/override-drops-synchronized` — an override that is not `synchronized` although the
//! method it replaces is.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["concurrency"],
    remediation_minutes: 5,
    description: r#"Detects a method overriding a `synchronized` method of a superclass declared
in the same file, without being `synchronized` itself.

**Why it matters.** `synchronized` is not inherited. The superclass made the method
thread-safe by locking the object, and its other synchronized methods rely on that lock; the
override silently runs without it, so it can interleave with them and see or leave
half-updated state.

**How to fix.** Declare the override `synchronized` too (or wrap its body in
`synchronized (this)`), or document why the lock is not needed.

```java
// Noncompliant
class Counter {
    protected int count;
    public synchronized void increment() { count++; }
}
class LoggingCounter extends Counter {
    @Override public void increment() { log(); count++; }
}

// Compliant
class LoggingCounter extends Counter {
    @Override public synchronized void increment() { log(); count++; }
}
```

Not reported: overrides whose body only delegates to `super` (the superclass method takes
the lock), only throws, only returns a literal, is empty, or is entirely a
`synchronized (this)` block — or ends with one, after statements that assign no field (a
read-only check before taking the lock: `verify(file); synchronized (this) { … }`); static
methods; superclasses declared in other files."#,
    ..RuleMeta::new(
        "java/override-drops-synchronized",
        "java",
        "Override drops synchronized",
    )
};

#[derive(Default)]
pub struct OverrideDropsSynchronized<'t> {
    /// Named classes of the file with a body.
    classes: Vec<(Node<'t>, &'t str)>,
    /// Classes (named or anonymous) that have a superclass, with their superclass type.
    subclasses: Vec<(Node<'t>, Node<'t>, Node<'t>)>,
    gate: util::Gate,
}

impl<'t> JavaRule<'t> for OverrideDropsSynchronized<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["class_declaration", "object_creation_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !self.gate.open(ctx, "synchronized") {
            return;
        }
        if kind(node) == "class_declaration" {
            let Some(body) = node.field(Field::Body) else {
                return;
            };
            self.classes
                .push((node, syntax::name_of(node, ctx.source())));
            if let Some(ty) = node
                .field(Field::Superclass)
                .and_then(syntax::first_named_child)
            {
                self.subclasses.push((node, body, ty));
            }
        } else if let Some(body) = syntax::child_of_kind(node, "class_body") {
            if let Some(ty) = node.field(Field::Type) {
                self.subclasses.push((node, body, ty));
            }
        }
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        let src = ctx.source();
        for &(_, body, sup_ty) in &self.subclasses {
            let Some(sup) = self.same_file_class(ctx, sup_ty) else {
                continue;
            };
            for m in ctx.members(body) {
                if kind(m) != "method_declaration"
                    || syntax::has_modifier(m, "synchronized")
                    || syntax::has_modifier(m, "static")
                    || syntax::has_modifier(m, "abstract")
                {
                    continue;
                }
                let Some(mbody) = m.field(Field::Body) else {
                    continue;
                };
                let name = syntax::name_of(m, src);
                if trivial_body(ctx, mbody, name) {
                    continue;
                }
                let Some((parent, overridden)) = self.synchronized_in_chain(ctx, sup, m, name)
                else {
                    continue;
                };
                let Some(name_node) = m.field(Field::Name) else {
                    continue;
                };
                let parent_name = syntax::name_of(parent, src);
                ctx.emit(
                    name_node,
                    format!("Make this override `synchronized`: `{parent_name}.{name}` is synchronized, and the lock is not inherited"),
                )
                .secondary(overridden.field(Field::Name).unwrap_or(overridden), "the synchronized method it overrides");
            }
        }
    }
}

impl<'t> OverrideDropsSynchronized<'t> {
    /// The class declaration of this file a superclass type names.
    fn same_file_class(&self, ctx: &Ctx<'t>, ty: Node<'t>) -> Option<Node<'t>> {
        let base = if kind(ty) == "generic_type" {
            syntax::first_named_child(ty)?
        } else {
            ty
        };
        let name = util::last_segment(ctx.text(base));
        let &(decl, _) = self.classes.iter().find(|(_, n)| *n == name)?;
        let fqn = ctx.class_fqn(decl)?;
        let resolved = ctx.resolve_type(base)?;
        (resolved.fqn() == Some(fqn.as_str())).then_some(decl)
    }

    /// The synchronized method `m` overrides in `sup` or its same-file superclasses: same
    /// name and the same parameter types as written. Stops at the first declaration found.
    fn synchronized_in_chain(
        &self,
        ctx: &Ctx<'t>,
        sup: Node<'t>,
        m: Node<'t>,
        name: &str,
    ) -> Option<(Node<'t>, Node<'t>)> {
        let src = ctx.source();
        let params: Vec<&str> = param_types(ctx, m);
        let mut cur = sup;
        for _ in 0..8 {
            let body = cur.field(Field::Body)?;
            for pm in ctx.members(body) {
                if kind(pm) == "method_declaration"
                    && syntax::name_of(pm, src) == name
                    && param_types(ctx, pm) == params
                {
                    let private = syntax::has_modifier(pm, "private");
                    return (syntax::has_modifier(pm, "synchronized")
                        && !private
                        && !syntax::has_modifier(pm, "static"))
                    .then_some((cur, pm));
                }
            }
            let next_ty = cur
                .field(Field::Superclass)
                .and_then(syntax::first_named_child)?;
            cur = self.same_file_class(ctx, next_ty)?;
        }
        None
    }
}

/// The parameter types of a method as written, without whitespace differences mattering.
fn param_types<'t>(ctx: &Ctx<'t>, m: Node<'t>) -> Vec<&'t str> {
    syntax::parameters(m)
        .map(|p| {
            p.field(Field::Type)
                .or_else(|| syntax::first_named_child(p))
                .map(|t| ctx.text(t))
                .unwrap_or("")
        })
        .collect()
}

/// Bodies that need no lock: empty, a single `super.name(..)` delegation, a single `throw`, a
/// single `return` of a literal, or a single `synchronized (this)` block.
fn trivial_body<'t>(ctx: &Ctx<'t>, body: Node<'t>, name: &str) -> bool {
    let mut stmts = named_children(body).filter(|n| !syntax::is_comment(*n));
    let Some(only) = stmts.next() else {
        return true;
    };
    if stmts.next().is_some() {
        return locks_this_after_read_only_check(ctx, body);
    }
    let expr = match kind(only) {
        "throw_statement" => return true,
        "synchronized_statement" => {
            return syntax::first_named_child(only)
                .map(syntax::strip_parens)
                .is_some_and(|l| kind(l) == "this");
        }
        "return_statement" | "expression_statement" => syntax::first_named_child(only),
        _ => None,
    };
    let Some(e) = expr else {
        return kind(only) == "return_statement";
    };
    let e = syntax::strip_parens(e);
    match kind(e) {
        "method_invocation" => {
            ctx.call_name(e) == Some(name)
                && e.field(Field::Object).is_some_and(|o| kind(o) == "super")
        }
        "decimal_integer_literal"
        | "string_literal"
        | "true"
        | "false"
        | "null_literal"
        | "character_literal"
        | "decimal_floating_point_literal" => true,
        _ => false,
    }
}

/// Whether the body ends with a `synchronized (this)` block and the statements before it only
/// read state (they assign no field: only locals and parameters): the override takes the
/// inherited lock itself around all the work that changes the object, after a check that
/// needs no lock (`verify(input); synchronized (this) { … }`).
fn locks_this_after_read_only_check<'t>(ctx: &Ctx<'t>, body: Node<'t>) -> bool {
    let stmts = || named_children(body).filter(|n| !syntax::is_comment(*n));
    let Some(last) = stmts().last() else {
        return false;
    };
    let locks_this = kind(last) == "synchronized_statement"
        && syntax::first_named_child(last)
            .map(syntax::strip_parens)
            .is_some_and(|l| kind(l) == "this");
    if !locks_this {
        return false;
    }
    stmts()
        .take_while(|s| s.id() != last.id())
        .all(|s| !writes_state(ctx, s))
}

/// Whether a statement assigns (or increments) anything but a local variable or parameter.
fn writes_state<'t>(ctx: &Ctx<'t>, stmt: Node<'t>) -> bool {
    let mut writes = false;
    util::visit(
        stmt,
        |_| true,
        |n| {
            let target = match kind(n) {
                "assignment_expression" => n.field(Field::Left),
                "update_expression" => syntax::first_named_child(n),
                _ => return,
            };
            let local = target.map(syntax::strip_parens).is_some_and(|t| {
                kind(t) == "identifier"
                    && ctx
                        .declaration_of(t)
                        .is_some_and(|d| !matches!(d.kind, VarKind::Field | VarKind::EnumConstant))
            });
            writes |= !local;
        },
    );
    writes
}
