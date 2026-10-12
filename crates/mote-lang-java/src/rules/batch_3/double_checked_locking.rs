//! `java/double-checked-locking` — lazy initialization of a non-volatile field with DCL.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["concurrency", "multi-threading"],
    remediation_minutes: 10,
    description: r#"Detects double-checked locking on a field that is not `volatile`:

```java
if (instance == null) {
    synchronized (lock) {
        if (instance == null) {
            instance = new Service();
        }
    }
}
```

**Why it matters.** Without `volatile` there is no happens-before edge between the thread that
writes the field inside the lock and a thread that reads it in the first, unsynchronized check.
The reader can see a non-null reference to an object whose constructor effects are not yet
visible, and use a half-initialized instance. The bug is rare, timing dependent, and very hard
to reproduce.

**How to fix.** Declare the field `volatile`, or use the initialization-on-demand holder idiom
(a `static` nested class holding the instance), or simply synchronize the whole accessor.

```java
// Compliant
private volatile Service instance;

// Compliant: holder idiom
private static final class Holder {
    static final Service INSTANCE = new Service();
}
```

Only fields declared in the same class are checked. Fields of type `String` or a boxed
primitive are not reported (their state is final, so publishing them through a data race is
safe), and neither is the local-variable form of the idiom."#,
    ..RuleMeta::new(
        "java/double-checked-locking",
        "java",
        "Double-checked locking on a non-volatile field",
    )
};

pub struct DoubleCheckedLocking;

impl<'t> JavaRule<'t> for DoubleCheckedLocking {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["synchronized_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        // The synchronized statement must be (in) the consequence of the outer null check.
        let Some(outer_if) = outer_if(node, ctx) else {
            return;
        };
        let Some((outer_field, outer_this)) =
            outer_if.field(Field::Condition).and_then(null_checked)
        else {
            return;
        };
        let field_name = ctx.text(outer_field);
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        let inner = util::statements(body).find(|s| {
            kind(*s) == "if_statement"
                && s.field(Field::Condition)
                    .and_then(null_checked)
                    .is_some_and(|(f, _)| ctx.text(f) == field_name)
        });
        let Some(inner_if) = inner else {
            return;
        };
        let Some(then) = inner_if.field(Field::Consequence) else {
            return;
        };
        let assigns = |n: Node<'t>| {
            kind(n) == "assignment_expression"
                && syntax::operator_str(n) == "="
                && n.field(Field::Left)
                    .and_then(util::simple_or_this_field)
                    .is_some_and(|(f, _)| ctx.text(f) == field_name)
        };
        if !(assigns(then) || util::any_descendant(then, util::is_other_context, assigns)) {
            return;
        }
        // Resolve the field.
        let (statement, type_node) = if outer_this {
            let Some(body) = ctx.enclosing_class(node).and_then(syntax::type_body) else {
                return;
            };
            let Some((decl, _)) = util::find_field(ctx, body, field_name) else {
                return;
            };
            (decl, decl.field(Field::Type))
        } else {
            let Some(d) = ctx.declaration_of(outer_field) else {
                return;
            };
            if d.kind != VarKind::Field {
                return;
            }
            (d.statement, d.type_node)
        };
        if syntax::has_modifier(statement, "volatile") || syntax::has_modifier(statement, "final") {
            return;
        }
        if let Some(t) = type_node.and_then(|t| ctx.resolve_type(t)) {
            if t.is_string() || t.is_boxed() || t.is_primitive() {
                return;
            }
        }
        let at = syntax::children(node).next().unwrap_or(node);
        ctx.emit(
            at,
            format!("Declare `{field_name}` volatile or use a holder class; double-checked locking on a non-volatile field is not thread-safe"),
        );
    }
}

/// The `if` statement whose consequence is (or directly contains) `sync`.
fn outer_if<'t>(sync: Node<'t>, ctx: &Ctx<'t>) -> Option<Node<'t>> {
    let mut cur = sync;
    let mut anc = ctx.ancestors(sync);
    let mut parent = anc.next()?;
    if kind(parent) == "block" {
        cur = parent;
        parent = anc.next()?;
    }
    if kind(parent) != "if_statement" {
        return None;
    }
    let is_then = parent
        .field(Field::Consequence)
        .is_some_and(|c| c.id() == cur.id());
    is_then.then_some(parent)
}

/// For a condition `(x == null)` / `(null == this.x)`: the field name node and whether it was
/// written `this.x`.
fn null_checked(cond: Node<'_>) -> Option<(Node<'_>, bool)> {
    let cond = syntax::strip_parens(cond);
    if kind(cond) != "binary_expression" || syntax::operator_str(cond) != "==" {
        return None;
    }
    let left = cond.field(Field::Left)?;
    let right = cond.field(Field::Right)?;
    let other = if syntax::is_null_literal(right) {
        left
    } else if syntax::is_null_literal(left) {
        right
    } else {
        return None;
    };
    util::simple_or_this_field(other)
}
