//! `java/loop-may-skip-bound` — a `for` loop tested with `!=` whose counter can jump over the
//! bound.

use super::util::{self, Walk};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    default_enabled: false,
    tags: &["loops", "suspicious"],
    remediation_minutes: 5,
    description: r#"Detects `for` loops whose condition is `i != bound` while the counter does not
move by exactly one each time: the update adds or subtracts a constant other than 1
(`i += 2`), or the body changes the counter as well.

**Why it matters.** With `!=` the loop only stops when the counter lands exactly on the
bound. A counter that moves in bigger steps, or is bumped again inside the body, can step
over it; the loop then runs on until the counter overflows or an index goes out of range.
`<` or `>` stop the loop whatever the step.

**How to fix.** Use an ordering comparison (`i < bound`, `i > bound`).

```java
// Noncompliant
for (int i = 0; i != data.length; i += 2) { … }   // odd length: never stops

// Compliant
for (int i = 0; i < data.length; i += 2) { … }
```

Off by default: `!=` loops are a style some code bases use on purpose. Not reported: loops
whose start and bound are constants that the step reaches exactly, and counters that are not
plain integer locals."#,
    ..RuleMeta::new(
        "java/loop-may-skip-bound",
        "java",
        "Loop tested with != may skip its bound",
    )
};

pub struct LoopMaySkipBound;

/// The constant step of an update of `var`: `i++` → 1, `i -= 3` → -3, `i = i + 2` → 2.
fn step<'t>(ctx: &Ctx<'t>, u: Node<'t>, var: &str) -> Option<i64> {
    let u = syntax::strip_parens(u);
    match kind(u) {
        "update_expression" => {
            let operand = syntax::first_named_child(u)?;
            if kind(operand) != "identifier" || ctx.text(operand) != var {
                return None;
            }
            Some(if ctx.text(u).contains("++") { 1 } else { -1 })
        }
        "assignment_expression" => {
            let left = u.field(Field::Left)?;
            if kind(left) != "identifier" || ctx.text(left) != var {
                return None;
            }
            let right = u.field(Field::Right)?;
            match syntax::operator_str(u) {
                "+=" => util_int(ctx, right),
                "-=" => util_int(ctx, right).map(|v| -v),
                "=" => {
                    let r = syntax::strip_parens(right);
                    if kind(r) != "binary_expression" {
                        return None;
                    }
                    let (l, rr) = (r.field(Field::Left)?, r.field(Field::Right)?);
                    if kind(l) != "identifier" || ctx.text(l) != var {
                        return None;
                    }
                    match syntax::operator_str(r) {
                        "+" => util_int(ctx, rr),
                        "-" => util_int(ctx, rr).map(|v| -v),
                        _ => None,
                    }
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn util_int(ctx: &Ctx<'_>, n: Node<'_>) -> Option<i64> {
    syntax::int_literal_value(n, ctx.source())
}

/// Whether the body assigns or increments `var`.
fn body_changes<'t>(ctx: &Ctx<'t>, body: Node<'t>, var: &str) -> bool {
    util::visit(body, |n| {
        if util::is_nested_scope(n) {
            return Walk::Skip;
        }
        let target = match kind(n) {
            "assignment_expression" => n.field(Field::Left),
            "update_expression" => syntax::first_named_child(n),
            _ => None,
        };
        if target
            .map(syntax::strip_parens)
            .is_some_and(|t| kind(t) == "identifier" && ctx.text(t) == var)
        {
            Walk::Stop
        } else {
            Walk::Continue
        }
    })
}

impl<'t> JavaRule<'t> for LoopMaySkipBound {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["for_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(cond) = node.field(Field::Condition).map(syntax::strip_parens) else {
            return;
        };
        if kind(cond) != "binary_expression" || syntax::operator_str(cond) != "!=" {
            return;
        }
        let (Some(l), Some(r)) = (cond.field(Field::Left), cond.field(Field::Right)) else {
            return;
        };
        let (l, r) = (syntax::strip_parens(l), syntax::strip_parens(r));
        // The counter: the side that the update changes.
        let updates: Vec<Node<'t>> = named_children(node)
            .filter(|c| {
                c.start_byte() > cond.end_byte()
                    && node.field(Field::Body).is_none_or(|b| c.id() != b.id())
            })
            .collect();
        if updates.len() != 1 {
            return;
        }
        let update = updates[0];
        let (var_node, bound) =
            if kind(l) == "identifier" && step(ctx, update, ctx.text(l)).is_some() {
                (l, r)
            } else if kind(r) == "identifier" && step(ctx, update, ctx.text(r)).is_some() {
                (r, l)
            } else {
                return;
            };
        if syntax::is_null_literal(bound) {
            return;
        }
        let var = ctx.text(var_node);
        let Some(delta) = step(ctx, update, var) else {
            return;
        };
        // The counter must be a local integer declared in the initializer.
        let init = named_children(node).find(|c| {
            kind(*c) == "local_variable_declaration"
                && syntax::declarators(*c).any(|d| syntax::name_of(d, ctx.source()) == var)
        });
        let Some(init) = init else { return };
        if !init
            .field(Field::Type)
            .is_some_and(|t| matches!(ctx.text(t), "int" | "long" | "short" | "byte"))
        {
            return;
        }
        let start = syntax::declarators(init)
            .find(|d| syntax::name_of(*d, ctx.source()) == var)
            .and_then(|d| d.field(Field::Value))
            .and_then(|v| util_int(ctx, v));
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        let reason = if delta.abs() != 1 {
            if let (Some(a), Some(b)) = (start, util_int(ctx, bound)) {
                let diff = b - a;
                if diff % delta == 0 && diff / delta >= 0 {
                    return;
                }
            }
            format!("steps by {}", delta.abs())
        } else if body_changes(ctx, body, var) {
            "is also changed in the body".to_string()
        } else {
            return;
        };
        ctx.emit(
            cond,
            format!("`{var}` {reason} and may jump over the bound: use `<` or `>` instead of `!=`"),
        );
    }
}
