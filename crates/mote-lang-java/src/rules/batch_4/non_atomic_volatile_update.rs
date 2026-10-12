//! `java/non-atomic-volatile-update` — `count++` / `count += n` on a `volatile` field.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["concurrency", "multi-threading"],
    remediation_minutes: 10,
    description: r#"Detects `++`, `--`, and compound assignments (`+=`, `|=`, …) applied to a
`volatile` field declared in this file.

**Why it matters.** `volatile` makes each individual read and write visible to other
threads, but `count++` is three steps — read, add, write. Two threads can read the same
value and both write back `value + 1`, losing an update. Counters, sequence numbers, and
flags updated this way drift under load, and the bug almost never reproduces in tests.

**How to fix.** Use an atomic type (`AtomicInteger.incrementAndGet()`,
`AtomicLong.addAndGet(n)`, `LongAdder` for hot counters), or guard every update with the
same lock.

```java
// Noncompliant
private volatile int requests;
void onRequest() { requests++; }

// Compliant
private final AtomicInteger requests = new AtomicInteger();
void onRequest() { requests.incrementAndGet(); }
```

Updates inside `synchronized` methods or blocks (volatile for lock-free readers, writers
serialized by the lock) and inside constructors and static initializers (the object is not
shared yet) are not reported."#,
    ..RuleMeta::new(
        "java/non-atomic-volatile-update",
        "java",
        "Non-atomic update of a volatile field",
    )
};

/// Names of the file's `volatile` fields (syntactic) and the updates of same-named targets;
/// `finish` resolves only those.
#[derive(Default)]
pub struct NonAtomicVolatileUpdate<'t> {
    volatile_names: FxHashSet<&'t str>,
    candidates: Vec<(Node<'t>, Node<'t>)>,
}

impl<'t> JavaRule<'t> for NonAtomicVolatileUpdate<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "update_expression",
            "assignment_expression",
            "field_declaration",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let target = match kind(node) {
            "field_declaration" => {
                if syntax::has_modifier(node, "volatile") {
                    for d in syntax::declarators(node) {
                        if let Some(n) = util::fld(d, F::Name) {
                            self.volatile_names.insert(ctx.text(n));
                        }
                    }
                }
                return;
            }
            "update_expression" => syntax::first_named_child(node),
            _ => {
                let op = util::op_str(node);
                if op == "=" || op.is_empty() {
                    return;
                }
                util::fld(node, F::Left)
            }
        };
        if let Some(target) = target {
            if matches!(kind(target), "identifier" | "field_access") {
                self.candidates.push((node, target));
            }
        }
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        if self.volatile_names.is_empty() {
            return;
        }
        let candidates = std::mem::take(&mut self.candidates);
        for (node, target) in candidates {
            if !util::var_name(ctx, target).is_some_and(|n| self.volatile_names.contains(n)) {
                continue;
            }
            let Some((decl, name)) = target_field(ctx, target) else {
                continue;
            };
            if !syntax::has_modifier(decl, "volatile") {
                continue;
            }
            if ctx.enclosing_function(node).is_none_or(|f| {
                matches!(
                    kind(f),
                    "constructor_declaration"
                        | "compact_constructor_declaration"
                        | "static_initializer"
                )
            }) {
                continue;
            }
            if util::in_synchronized(ctx, node) {
                continue;
            }
            ctx.emit(
                node,
                format!("Make this update of volatile field `{name}` atomic (an Atomic* class or a lock); `volatile` does not make read-modify-write atomic"),
            );
        }
    }
}

/// The field declaration a target (`x` or `this.x`) refers to.
fn target_field<'t>(ctx: &Ctx<'t>, target: Node<'t>) -> Option<(Node<'t>, &'t str)> {
    let target = syntax::strip_parens(target);
    match kind(target) {
        "identifier" => {
            let d = ctx.declaration_of(target)?;
            (d.kind == VarKind::Field).then(|| (d.statement, ctx.text(target)))
        }
        "field_access" => {
            let obj = util::fld(target, F::Object)?;
            let field = util::fld(target, F::Field)?;
            if kind(obj) != "this" || kind(field) != "identifier" {
                return None;
            }
            let name = ctx.text(field);
            let class = ctx.enclosing_class(target)?;
            let body = syntax::type_body(class)?;
            util::find_field(ctx, body, name).map(|(decl, _)| (decl, name))
        }
        _ => None,
    }
}
