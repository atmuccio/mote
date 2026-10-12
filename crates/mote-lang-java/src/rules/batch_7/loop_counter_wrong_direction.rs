//! `java/loop-counter-wrong-direction` — a `for` update that moves the counter away from the
//! bound its condition tests.

use super::util::{self, Walk};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["loops", "suspicious", "typo"],
    remediation_minutes: 2,
    description: r#"Detects a `for` loop whose update moves the counter away from the bound tested
by its condition: `i < n` with `i--`, or `i >= 0` with `i++` (also `+=` / `-=` by a constant
and `i = i + 1` forms).

**Why it matters.** The condition stays true while the counter runs off in the wrong
direction, so the loop does not stop where it should: it runs until the counter overflows,
indexes an array with a negative position, or never ends. It is almost always a slip made
while turning an ascending loop into a descending one (or the reverse) and changing only one
of the three clauses.

**How to fix.** Make the update and the condition agree.

```java
// Noncompliant
for (int i = items.length - 1; i < items.length; i--) { … }

// Compliant
for (int i = items.length - 1; i >= 0; i--) { … }
```

Only counters that are plain local names are considered. Loops whose body also assigns the
counter, conditions that are not `<`, `<=`, `>` or `>=`, steps that are not constants, and
loops whose update also changes the bound are not reported."#,
    ..RuleMeta::new(
        "java/loop-counter-wrong-direction",
        "java",
        "Loop counter moves away from its bound",
    )
};

pub struct LoopCounterWrongDirection;

/// The counter a `for` update changes and the sign of the change (`+1` / `-1`), when the step
/// is a constant.
fn update_step<'t>(ctx: &Ctx<'t>, u: Node<'t>) -> Option<(&'t str, i32)> {
    let u = syntax::strip_parens(u);
    match kind(u) {
        "update_expression" => {
            let operand = syntax::first_named_child(u)?;
            if kind(operand) != "identifier" {
                return None;
            }
            let text = ctx.text(u);
            let sign = if text.starts_with("++") || text.ends_with("++") {
                1
            } else if text.starts_with("--") || text.ends_with("--") {
                -1
            } else {
                return None;
            };
            Some((ctx.text(operand), sign))
        }
        "assignment_expression" => {
            let left = u.field(Field::Left)?;
            let right = u.field(Field::Right)?;
            if kind(left) != "identifier" {
                return None;
            }
            let name = ctx.text(left);
            let sign = match syntax::operator_str(u) {
                "+=" => constant_sign(ctx, right)?,
                "-=" => -constant_sign(ctx, right)?,
                "=" => {
                    let r = syntax::strip_parens(right);
                    if kind(r) != "binary_expression" {
                        return None;
                    }
                    let (Some(rl), Some(rr)) = (r.field(Field::Left), r.field(Field::Right)) else {
                        return None;
                    };
                    if kind(rl) != "identifier" || ctx.text(rl) != name {
                        return None;
                    }
                    match syntax::operator_str(r) {
                        "+" => constant_sign(ctx, rr)?,
                        "-" => -constant_sign(ctx, rr)?,
                        _ => return None,
                    }
                }
                _ => return None,
            };
            Some((name, sign))
        }
        _ => None,
    }
}

/// `+1` / `-1` for a nonzero integer literal, `None` otherwise.
fn constant_sign(ctx: &Ctx<'_>, node: Node<'_>) -> Option<i32> {
    match util::int_value(ctx, node)? {
        0 => None,
        v if v > 0 => Some(1),
        _ => Some(-1),
    }
}

/// Whether `node` mentions the identifier `name`.
fn mentions<'t>(ctx: &Ctx<'t>, node: Node<'t>, name: &str) -> bool {
    if !ctx.text(node).contains(name) {
        return false;
    }
    (kind(node) == "identifier" && ctx.text(node) == name)
        || util::visit(node, |n| {
            if kind(n) == "identifier" && ctx.text(n) == name {
                Walk::Stop
            } else {
                Walk::Continue
            }
        })
}

/// Whether the body assigns or increments `name`.
fn body_writes<'t>(ctx: &Ctx<'t>, body: Node<'t>, name: &str) -> bool {
    if !ctx.text(body).contains(name) {
        return false;
    }
    util::visit(body, |n| {
        let target = match kind(n) {
            "assignment_expression" => n.field(Field::Left),
            "update_expression" => syntax::first_named_child(n),
            _ => None,
        };
        match target {
            Some(t) if kind(t) == "identifier" && ctx.text(t) == name => Walk::Stop,
            _ => Walk::Continue,
        }
    })
}

impl<'t> JavaRule<'t> for LoopCounterWrongDirection {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["for_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(cond) = node.field(Field::Condition) else {
            return;
        };
        let cond = syntax::strip_parens(cond);
        if kind(cond) != "binary_expression" {
            return;
        }
        let op = syntax::operator_str(cond);
        // +1: the side's value must grow for the condition to become false.
        let left_must = match op {
            "<" | "<=" => 1,
            ">" | ">=" => -1,
            _ => return,
        };
        let (Some(left), Some(right)) = (cond.field(Field::Left), cond.field(Field::Right)) else {
            return;
        };
        // All updates; bail on any we don't understand.
        let mut steps: Vec<(&'t str, i32, Node<'t>)> = Vec::new();
        let mut cursor = node.walk();
        for u in node.children_by_field_name("update", &mut cursor) {
            if syntax::is_comment(u) {
                continue;
            }
            let Some((name, sign)) = update_step(ctx, u) else {
                return;
            };
            steps.push((name, sign, u));
        }
        for (counter, bound, must) in [(left, right, left_must), (right, left, -left_must)] {
            let counter = syntax::strip_parens(counter);
            if kind(counter) != "identifier" {
                continue;
            }
            let name = ctx.text(counter);
            let mut mine = steps.iter().filter(|(n, _, _)| *n == name);
            let (Some(&(_, sign, update)), None) = (mine.next(), mine.next()) else {
                continue;
            };
            if sign == must {
                continue;
            }
            if mentions(ctx, bound, name) || steps.iter().any(|(n, _, _)| mentions(ctx, bound, n)) {
                continue;
            }
            if node
                .field(Field::Body)
                .is_some_and(|b| body_writes(ctx, b, name))
            {
                continue;
            }
            let dir = if sign > 0 { "increases" } else { "decreases" };
            ctx.emit(
                update,
                format!(
                    "Fix the loop update: it {dir} `{name}`, which moves it away from the bound tested by `{}`",
                    ctx.text(cond)
                ),
            );
            return;
        }
    }
}
