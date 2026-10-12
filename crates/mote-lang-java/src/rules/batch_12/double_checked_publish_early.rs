//! `java/double-checked-publish-early` — double-checked initialization that stores the new
//! object in the field and only then finishes setting it up.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["concurrency", "multi-threading"],
    remediation_minutes: 10,
    description: r#"Detects double-checked lazy initialization (`if (f == null) { synchronized (…) {
if (f == null) { … } } }`) where the field is assigned first and the new object is then set
up through the field (`f = new Registry(); f.load();`, `f = new HashMap<>(); f.put(…)`).

**Why it matters.** The first, unsynchronized check lets other threads use the field as soon as
it is non-null. Assigning it before the set-up finishes publishes a half-built object: another
thread skips the lock, sees the field is set, and works with an object that is not loaded or
filled yet. `volatile` does not help, since the object really is incomplete at the moment it
becomes visible.

**How to fix.** Build and set up the object in a local variable, and assign the field last.

```java
// Noncompliant
if (registry == null) {
    synchronized (this) {
        if (registry == null) {
            registry = new Registry();
            registry.load(config);
        }
    }
}

// Compliant
if (registry == null) {
    synchronized (this) {
        if (registry == null) {
            Registry r = new Registry();
            r.load(config);
            registry = r;
        }
    }
}
```

Only statements that directly follow the assignment inside the inner check are considered: a
call on the field (`f.load()`) or a write to one of its fields (`f.size = 3`)."#,
    ..RuleMeta::new(
        "java/double-checked-publish-early",
        "java",
        "Double-checked initialization publishes the object before it is set up",
    )
};

pub struct DoubleCheckedPublishEarly;

impl<'t> JavaRule<'t> for DoubleCheckedPublishEarly {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["synchronized_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(outer_if) = enclosing_then_if(node, ctx) else {
            return;
        };
        let Some(field) = outer_if
            .field(Field::Condition)
            .and_then(|c| null_checked(c, ctx))
        else {
            return;
        };
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        let inner = named_children(body).find(|s| {
            kind(*s) == "if_statement"
                && s.field(Field::Condition).and_then(|c| null_checked(c, ctx)) == Some(field)
        });
        let Some(then) = inner.and_then(|i| i.field(Field::Consequence)) else {
            return;
        };
        if kind(then) != "block" {
            return;
        }
        let mut assignment = None;
        for stmt in named_children(then) {
            if let Some(assign) = assignment {
                if sets_up(stmt, field, ctx) && is_field(assign, ctx) {
                    ctx.emit(
                        assign,
                        format!("Set up the new object in a local variable and assign `{field}` last; other threads can see it before it is ready"),
                    )
                    .secondary(stmt, "set up after publication");
                    return;
                }
                continue;
            }
            if kind(stmt) != "expression_statement" {
                continue;
            }
            let Some(e) = syntax::first_named_child(stmt) else {
                continue;
            };
            if kind(e) == "assignment_expression"
                && syntax::operator_str(e) == "="
                && e.field(Field::Left).and_then(|l| field_name(l, ctx)) == Some(field)
                && e.field(Field::Right)
                    .is_some_and(|r| kind(syntax::strip_parens(r)) == "object_creation_expression")
            {
                assignment = Some(e);
            }
        }
    }
}

/// The `if` statement whose consequence is, or is a block directly holding, `sync`.
fn enclosing_then_if<'t>(sync: Node<'t>, ctx: &Ctx<'t>) -> Option<Node<'t>> {
    let mut anc = ctx.ancestors(sync);
    let mut cur = sync;
    let mut parent = anc.next()?;
    if kind(parent) == "block" {
        cur = parent;
        parent = anc.next()?;
    }
    (kind(parent) == "if_statement"
        && parent
            .field(Field::Consequence)
            .is_some_and(|c| c.id() == cur.id()))
    .then_some(parent)
}

/// `x` for `x` or `this.x`.
fn field_name<'t>(expr: Node<'t>, ctx: &Ctx<'t>) -> Option<&'t str> {
    let e = syntax::strip_parens(expr);
    match kind(e) {
        "identifier" => Some(ctx.text(e)),
        "field_access" if e.field(Field::Object).is_some_and(|o| kind(o) == "this") => {
            e.field(Field::Field).map(|f| ctx.text(f))
        }
        _ => None,
    }
}

/// The name tested by `(x == null)` / `(null == this.x)`.
fn null_checked<'t>(cond: Node<'t>, ctx: &Ctx<'t>) -> Option<&'t str> {
    let cond = syntax::strip_parens(cond);
    if kind(cond) != "binary_expression" || syntax::operator_str(cond) != "==" {
        return None;
    }
    let (left, right) = (cond.field(Field::Left)?, cond.field(Field::Right)?);
    let other = if syntax::is_null_literal(right) {
        left
    } else if syntax::is_null_literal(left) {
        right
    } else {
        return None;
    };
    field_name(other, ctx)
}

/// `field.m(…);` or `field.x = …;`
fn sets_up<'t>(stmt: Node<'t>, field: &str, ctx: &Ctx<'t>) -> bool {
    if kind(stmt) != "expression_statement" {
        return false;
    }
    let Some(e) = syntax::first_named_child(stmt) else {
        return false;
    };
    let receiver = match kind(e) {
        "method_invocation" => e.field(Field::Object),
        "assignment_expression" => e
            .field(Field::Left)
            .filter(|l| kind(*l) == "field_access")
            .and_then(|l| l.field(Field::Object)),
        _ => None,
    };
    receiver.and_then(|r| field_name(r, ctx)) == Some(field)
}

/// Whether the target of `assign` is a field (`this.x`, or a name resolving to a field).
fn is_field<'t>(assign: Node<'t>, ctx: &Ctx<'t>) -> bool {
    let Some(left) = assign.field(Field::Left).map(syntax::strip_parens) else {
        return false;
    };
    match kind(left) {
        "field_access" => true,
        "identifier" => ctx
            .declaration_of(left)
            .is_some_and(|d| d.kind == VarKind::Field),
        _ => false,
    }
}
