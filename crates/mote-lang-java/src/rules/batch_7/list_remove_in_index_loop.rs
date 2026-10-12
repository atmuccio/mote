//! `java/list-remove-in-index-loop` — `list.remove(i)` in an ascending `for` loop over `i`
//! skips the element after each removed one.

use super::util::{self, Walk};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["collections", "loops"],
    remediation_minutes: 5,
    description: r#"Detects `list.remove(i)` inside a `for` loop that counts `i` upwards (`i++`),
when nothing compensates for the removal.

**Why it matters.** Removing the element at `i` shifts every following element one position
to the left. The loop then increments `i` and never looks at the element that moved into
position `i`: two matching elements in a row leave the second one in the list. The bug only
shows with adjacent matches, so tests with sparse data pass.

**How to fix.** Use `removeIf`, an explicit `Iterator.remove()`, or iterate backwards.

```java
// Noncompliant
for (int i = 0; i < names.size(); i++) {
    if (names.get(i).isBlank()) {
        names.remove(i);
    }
}

// Compliant
names.removeIf(String::isBlank);
```

Not reported: loops whose body also changes `i` (the `i--` compensation), removals followed by
`break`, `return` or `throw` in the same block, and receivers or counters whose types are not
proven (`List` and `int`; with an `Integer` counter `remove` removes by value)."#,
    ..RuleMeta::new(
        "java/list-remove-in-index-loop",
        "java",
        "List element removed in an ascending index loop",
    )
};

pub struct ListRemoveInIndexLoop;

/// The counter of `for (…; i < …; i++)` (also `++i`, `i += 1`).
fn ascending_counter<'t>(ctx: &Ctx<'t>, node: Node<'t>) -> Option<&'t str> {
    let cond = syntax::strip_parens(node.field(Field::Condition)?);
    if kind(cond) != "binary_expression" || !matches!(syntax::operator_str(cond), "<" | "<=") {
        return None;
    }
    let left = syntax::strip_parens(cond.field(Field::Left)?);
    if kind(left) != "identifier" {
        return None;
    }
    let name = ctx.text(left);
    let mut cursor = node.walk();
    let mut updates = node
        .children_by_field_name("update", &mut cursor)
        .filter(|u| !syntax::is_comment(*u));
    let (Some(u), None) = (updates.next(), updates.next()) else {
        return None;
    };
    let text = ctx.text(u);
    let ok = match kind(u) {
        "update_expression" => text == format!("{name}++") || text == format!("++{name}"),
        "assignment_expression" => {
            syntax::operator_str(u) == "+="
                && u.field(Field::Left).is_some_and(|l| ctx.text(l) == name)
                && u.field(Field::Right)
                    .is_some_and(|r| util::int_value(ctx, r) == Some(1))
        }
        _ => false,
    };
    ok.then_some(name)
}

impl<'t> JavaRule<'t> for ListRemoveInIndexLoop {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["for_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        if !ctx.text(body).contains(".remove(") {
            return;
        }
        let Some(counter) = ascending_counter(ctx, node) else {
            return;
        };
        // One pass over the body: find `x.remove(counter)`, note writes to the counter and
        // whether the removing statement is followed by an exit in its block.
        let mut removal: Option<Node<'t>> = None;
        let mut compensated = kind(body) == "block" && exits_after_removal(ctx, body, counter);
        util::visit(body, |n| {
            if util::is_nested_scope(n) {
                return Walk::Skip;
            }
            match kind(n) {
                "assignment_expression" => {
                    if n.field(Field::Left).is_some_and(|l| ctx.text(l) == counter) {
                        compensated = true;
                        return Walk::Stop;
                    }
                }
                "update_expression" => {
                    if syntax::first_named_child(n).is_some_and(|o| ctx.text(o) == counter) {
                        compensated = true;
                        return Walk::Stop;
                    }
                }
                "block" => {
                    if exits_after_removal(ctx, n, counter) {
                        compensated = true;
                        return Walk::Stop;
                    }
                }
                "method_invocation" => {
                    if removal.is_none() && is_removal(ctx, n, counter) {
                        removal = Some(n);
                    } else if ctx.call_name(n) == Some("add")
                        && util::arg_count(n) == 2
                        && util::arg(n, 0).is_some_and(|a| ctx.text(a) == counter)
                    {
                        // `remove(i)` + `add(i, x)` replaces in place: nothing shifts.
                        compensated = true;
                        return Walk::Stop;
                    }
                }
                "return_statement" | "throw_statement"
                    if removal.is_none()
                        && ctx.text(n).contains(".remove(")
                        && util::any_in_scope(n, |c| is_removal(ctx, c, counter)) =>
                {
                    compensated = true;
                    return Walk::Stop;
                }
                _ => {}
            }
            Walk::Continue
        });
        if compensated {
            return;
        }
        let Some(call) = removal else {
            return;
        };
        // Types last: `List` receiver, `int` index.
        if !util::receiver_is(ctx, call, &["java.util.List"]) {
            return;
        }
        let Some(arg) = util::arg(call, 0) else {
            return;
        };
        if ctx.type_of(arg).and_then(|t| t.prim()) != Some(Prim::Int) {
            return;
        }
        ctx.emit(
            call,
            format!("Removing at `{counter}` shifts the next element into position `{counter}`, which the loop then skips; use removeIf or an Iterator"),
        );
    }
}

fn is_removal<'t>(ctx: &Ctx<'t>, n: Node<'t>, counter: &str) -> bool {
    if kind(n) != "method_invocation" || ctx.call_name(n) != Some("remove") {
        return false;
    }
    if n.field(Field::Object).is_none() {
        return false;
    }
    let mut args = util::args(n);
    matches!(
        (args.next(), args.next()),
        (Some(a), None) if kind(syntax::strip_parens(a)) == "identifier" && ctx.text(syntax::strip_parens(a)) == counter
    )
}

fn is_removal_stmt<'t>(ctx: &Ctx<'t>, stmt: Node<'t>, counter: &str) -> bool {
    ctx.text(stmt).contains(".remove(")
        && (is_removal(ctx, stmt, counter)
            || util::any_in_scope(stmt, |c| is_removal(ctx, c, counter)))
}

/// Whether a statement of `block` removes at the counter and a later statement of the same
/// block leaves the loop.
fn exits_after_removal<'t>(ctx: &Ctx<'t>, block: Node<'t>, counter: &str) -> bool {
    let mut seen_remove = false;
    for s in named_children(block) {
        if seen_remove
            && matches!(
                kind(s),
                "break_statement" | "return_statement" | "throw_statement"
            )
        {
            return true;
        }
        if !seen_remove && is_removal_stmt(ctx, s, counter) {
            seen_remove = true;
        }
    }
    false
}
