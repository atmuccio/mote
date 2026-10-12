//! `java/synchronized-on-reassigned-field` — `synchronized (field) { … field = …; }`: a block
//! that replaces the very object it locks.

use super::util;
use crate::prelude::*;
use crate::rules::shared::util::CoveredRanges;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["concurrency", "multi-threading"],
    remediation_minutes: 10,
    description: r#"Detects a `synchronized (field)` block that assigns a new object to that same
field.

**Why it matters.** A `synchronized` block locks the object the field refers to when the
block starts. Once the block replaces the field's value, the next thread locks the *new*
object and enters immediately, even though the first thread is still inside the block holding
the old one. The mutual exclusion the block was written for is gone precisely when the guarded
state changes.

**How to fix.** Lock a dedicated `final` object and guard the mutable reference with it.

```java
// Noncompliant
synchronized (listeners) {
    listeners = new ArrayList<>(listeners);
    listeners.add(l);
}

// Compliant
private final Object listenersLock = new Object();
synchronized (listenersLock) {
    listeners = new ArrayList<>(listeners);
    listeners.add(l);
}
```

Only fields (plain names resolving to a field, or `this.field`) are checked. Assignments of
`null` and assignments inside lambdas or nested classes declared in the block are not
reported."#,
    ..RuleMeta::new(
        "java/synchronized-on-reassigned-field",
        "java",
        "Synchronized block replaces its own lock",
    )
};

#[derive(Default)]
pub struct SynchronizedOnReassignedField<'t> {
    /// The assignments a lock could be replaced by (see [`Assignments`]) in the `synchronized`
    /// bodies indexed so far ([`Self::covered`]): a body is indexed by the first statement that
    /// needs it, and an enclosing statement's index already holds a nested body's.
    assignments: Assignments<'t>,
    covered: CoveredRanges,
}

/// Plain `=` assignments (of the `synchronized` bodies indexed so far) whose value is not `null` and whose target is a
/// variable or a `this.` field, by target name, in source (pre-order) order, with the start of its
/// nearest enclosing nested scope (lambda, class body, type declaration). Each `synchronized`
/// statement looks up its body's range here instead of visiting the body, which is quadratic
/// when such statements nest.
type Assignments<'t> = FxHashMap<&'t str, Vec<Assignment<'t>>>;

struct Assignment<'t> {
    start: usize,
    left: Node<'t>,
    /// Start of the nearest enclosing nested scope ([`util::is_nested_scope`]).
    scope: Option<usize>,
}

fn collect_assignments<'t>(ctx: &Ctx<'t>, body: Node<'t>, out: &mut Assignments<'t>) {
    // Pre-order walk keeping the stack of enclosing nested scopes. Scopes enclosing `body` are
    // not on it: an assignment whose nearest nested scope lies outside `body` gets no scope
    // instead of that scope's start, and both pass the test of any body inside `body` (which
    // only looks for scopes starting inside the body asked about).
    let mut scopes: Vec<(usize, usize)> = Vec::new(); // (start, end) of open nested scopes
    let mut cursor = body.walk();
    'walk: loop {
        let n = cursor.node();
        if n.is_named() {
            while scopes.last().is_some_and(|&(_, end)| n.start_byte() >= end) {
                scopes.pop();
            }
            if kind(n) == "assignment_expression" && syntax::operator_str(n) == "=" {
                if let (Some(left), Some(right)) = (n.field(Field::Left), n.field(Field::Right)) {
                    if !syntax::is_null_literal(right) {
                        if let Some(name) = util::var_name(ctx, left) {
                            out.entry(name).or_default().push(Assignment {
                                start: n.start_byte(),
                                left,
                                scope: scopes.last().map(|&(start, _)| start),
                            });
                        }
                    }
                }
            }
            if util::is_nested_scope(n) {
                scopes.push((n.start_byte(), n.end_byte()));
            }
        }
        if cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                continue 'walk;
            }
            if !cursor.goto_parent() {
                break 'walk;
            }
        }
    }
}

/// Whether `lock` (a lock expression) names a field of this file.
fn is_field<'t>(ctx: &Ctx<'t>, lock: Node<'t>) -> bool {
    match kind(lock) {
        "identifier" => ctx
            .declaration_of(lock)
            .is_some_and(|d| d.kind == VarKind::Field),
        "field_access" => lock.field(Field::Object).is_some_and(|o| kind(o) == "this"),
        _ => false,
    }
}

impl<'t> JavaRule<'t> for SynchronizedOnReassignedField<'t> {
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
        let Some(name) = util::var_name(ctx, lock) else {
            return;
        };
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        // The first assignment to `name` in the body, outside the lambdas and classes nested in
        // it (one whose nearest nested scope starts before the body encloses the whole block).
        let (from, to) = (body.start_byte(), body.end_byte());
        if from >= to {
            return;
        }
        if !self.covered.contains(from, to) {
            let in_order = self.covered.add(from, to);
            collect_assignments(ctx, body, &mut self.assignments);
            if !in_order {
                // A body before one indexed earlier (a `synchronized` inside the lock
                // expression of another): keep every list in source order.
                for list in self.assignments.values_mut() {
                    list.sort_by_key(|a| a.start);
                }
            }
        }
        let target = self.assignments.get(name).and_then(|list| {
            let first = list.partition_point(|a| a.start < from);
            list[first..]
                .iter()
                .take_while(|a| a.start < to)
                .find(|a| a.scope.is_none_or(|s| s < from))
                .map(|a| syntax::strip_parens(a.left))
        });
        let Some(target) = target else {
            return;
        };
        // Both must be the same field: a local or parameter of that name shadows it.
        if !is_field(ctx, lock) {
            return;
        }
        if kind(target) == "identifier" && !is_field(ctx, target) {
            return;
        }
        ctx.emit(
            lock,
            format!("Lock a dedicated final object: this block replaces `{name}`, so the next thread locks the new object while this one still holds the old"),
        )
        .secondary(target, "replaced here");
    }
}
