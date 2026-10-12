//! `java/linear-access-in-loop` — list operations that cost a walk or a shift of the whole list,
//! repeated in a loop: `LinkedList.get(i)`, `ArrayList.remove(0)`, `ArrayList.add(0, x)`.

use super::common::{self, Walk};
use super::util::Gate;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["performance", "collections"],
    remediation_minutes: 10,
    description: r#"Detects, inside a loop, list operations whose cost grows with the size of the
list:

* `get(i)` on a `LinkedList`, with `i` the counter of the enclosing `for` loop: every call walks
  the nodes from one end of the list up to `i`;
* `remove(0)` or `add(0, x)` on an `ArrayList`: every call shifts all other elements by one
  place.

The list's class must be known from its declaration: a variable or field declared as
`LinkedList` / `ArrayList`, or declared with an interface type, initialized with `new` of that
class and never reassigned (a local) or `final` (a field).

**Why it matters.** Each call is linear in the size of the list, so the loop is quadratic: a
loop over 100 000 elements performs billions of steps where a linear pass would do. The cost
stays invisible in tests with small lists and appears in production.

**How to fix.** Iterate a `LinkedList` with a for-each loop or an iterator. Use an
`ArrayDeque` for a queue (`poll()`, `addFirst(x)`), or build the list in the other order and
reverse it once.

```java
// Noncompliant
for (int i = 0; i < names.size(); i++) {   // names is a LinkedList
    print(names.get(i));
}
while (!pending.isEmpty()) {               // pending is an ArrayList
    handle(pending.remove(0));
}

// Compliant
for (String name : names) {
    print(name);
}
Deque<Task> pending = new ArrayDeque<>(tasks);
while (!pending.isEmpty()) {
    handle(pending.poll());
}
```

Not reported: `get(0)` and other constant positions of a `LinkedList` (they are found at once),
lists whose class is not known from the declaration, calls in a lambda or class declared in
the loop, and calls followed by a `break`, `return` or `throw` in the same block (they run at
most once per loop)."#,
    ..RuleMeta::new(
        "java/linear-access-in-loop",
        "java",
        "Linear-time list access inside a loop",
    )
};

#[derive(Default)]
pub struct LinearAccessInLoop {
    linked: Gate,
    array: Gate,
}

const LINKED_LIST: &str = "java.util.LinkedList";
const ARRAY_LIST: &str = "java.util.ArrayList";

fn is_loop(n: Node<'_>) -> bool {
    matches!(
        kind(n),
        "for_statement" | "enhanced_for_statement" | "while_statement" | "do_statement"
    )
}

/// The concrete list class of a receiver, when its declaration proves it.
fn list_class<'t>(ctx: &Ctx<'t>, recv: Node<'t>) -> Option<&'static str> {
    let recv = syntax::strip_parens(recv);
    if kind(recv) != "identifier" {
        return None;
    }
    let pick = |fqn: &str| match fqn {
        LINKED_LIST => Some(LINKED_LIST),
        ARRAY_LIST => Some(ARRAY_LIST),
        _ => None,
    };
    let t = ctx.type_of(recv)?;
    if let Some(c) = t.fqn().and_then(pick) {
        return Some(c);
    }
    // Declared with an interface type: the initializer decides, if the variable keeps it.
    let decl = ctx.declaration_of(recv)?;
    let value = syntax::strip_parens(decl.value?);
    if kind(value) != "object_creation_expression"
        || syntax::child_of_kind(value, "class_body").is_some()
    {
        return None;
    }
    let created = ctx.resolve_type(value.field(Field::Type)?)?;
    let class = created.fqn().and_then(pick)?;
    let name = ctx.text(recv);
    match decl.kind {
        VarKind::Field => syntax::has_modifier(decl.statement, "final").then_some(class),
        VarKind::Local => {
            let body = ctx.enclosing_function(recv)?;
            (!reassigned(ctx, body, name)).then_some(class)
        }
        _ => None,
    }
}

/// Whether `name` is assigned anywhere in `scope` (after its declaration).
fn reassigned<'t>(ctx: &Ctx<'t>, scope: Node<'t>, name: &str) -> bool {
    common::walk(scope, |n| {
        if kind(n) == "assignment_expression"
            && n.field(Field::Left)
                .is_some_and(|l| kind(l) == "identifier" && ctx.text(l) == name)
        {
            Walk::Stop
        } else {
            Walk::Continue
        }
    })
}

/// The innermost loop around `node` in the same function, and whether the path to it runs
/// through the loop's body (or condition).
fn enclosing_loop<'t>(ctx: &Ctx<'t>, node: Node<'t>) -> Option<Node<'t>> {
    let mut prev = node;
    for a in ctx.ancestors(node) {
        if common::is_other_context(a) || syntax::is_method_like(a) {
            return None;
        }
        if is_loop(a) {
            // Not the iterable of a for-each or the init of a for: those run once.
            let once = match kind(a) {
                "enhanced_for_statement" => {
                    a.field(Field::Value).map(|v| v.id()) == Some(prev.id())
                }
                "for_statement" => a.field(Field::Init).map(|v| v.id()) == Some(prev.id()),
                _ => false,
            };
            if !once {
                return Some(a);
            }
        }
        prev = a;
    }
    None
}

/// Whether the statement holding `node` is followed, in its block, by a jump out of the loop.
fn runs_once<'t>(ctx: &Ctx<'t>, node: Node<'t>, lp: Node<'t>) -> bool {
    let mut prev = node;
    for a in ctx.ancestors(node) {
        if a.id() == lp.id() {
            return false;
        }
        if matches!(kind(a), "block" | "switch_block_statement_group") {
            let mut after = false;
            for s in named_children(a) {
                if s.id() == prev.id() {
                    after = true;
                    continue;
                }
                if after
                    && matches!(
                        kind(s),
                        "break_statement" | "return_statement" | "throw_statement"
                    )
                {
                    return true;
                }
            }
        }
        prev = a;
    }
    false
}

/// The counter variables of a `for` statement (declared or assigned in its init).
fn for_counters<'t>(ctx: &Ctx<'t>, lp: Node<'t>) -> Vec<&'t str> {
    let mut out = Vec::new();
    let mut cursor = lp.walk();
    for init in lp.children_by_field_name("init", &mut cursor) {
        match kind(init) {
            "local_variable_declaration" => {
                for d in syntax::declarators(init) {
                    if let Some(n) = d.field(Field::Name) {
                        out.push(ctx.text(n));
                    }
                }
            }
            "assignment_expression" => {
                if let Some(l) = init.field(Field::Left).filter(|l| kind(*l) == "identifier") {
                    out.push(ctx.text(l));
                }
            }
            _ => {}
        }
    }
    out
}

/// Whether `index` is the counter of an enclosing `for` loop (`i`, `i + 1`, `i - 1`).
fn is_loop_counter<'t>(ctx: &Ctx<'t>, index: Node<'t>, call: Node<'t>) -> bool {
    let index = syntax::strip_parens(index);
    let var = match kind(index) {
        "identifier" => index,
        "binary_expression" if matches!(syntax::operator_str(index), "+" | "-") => {
            let (Some(l), Some(r)) = (index.field(Field::Left), index.field(Field::Right)) else {
                return false;
            };
            if kind(l) != "identifier" || syntax::int_literal_value(r, ctx.source()).is_none() {
                return false;
            }
            l
        }
        _ => return false,
    };
    let name = ctx.text(var);
    for a in ctx.ancestors(call) {
        if common::is_other_context(a) || syntax::is_method_like(a) {
            return false;
        }
        if kind(a) == "for_statement" && for_counters(ctx, a).contains(&name) {
            return true;
        }
    }
    false
}

impl<'t> JavaRule<'t> for LinearAccessInLoop {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["get", "remove", "add"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) = ctx.call_name(node) else {
            return;
        };
        let n = syntax::argument_count(node);
        let src = ctx.source();
        let first = || syntax::arguments(node).next().map(syntax::strip_parens);
        let wanted = match (name, n) {
            ("get", 1) => {
                if !self.linked.open(ctx, "LinkedList") {
                    return;
                }
                LINKED_LIST
            }
            ("remove", 1) | ("add", 2) => {
                if !self.array.open(ctx, "ArrayList") {
                    return;
                }
                if !first().is_some_and(|a| {
                    kind(a) == "decimal_integer_literal"
                        && syntax::int_literal_value(a, src) == Some(0)
                }) {
                    return;
                }
                ARRAY_LIST
            }
            _ => return,
        };
        let Some(recv) = node.field(Field::Object) else {
            return;
        };
        if kind(recv) != "identifier" {
            return;
        }
        let Some(lp) = enclosing_loop(ctx, node) else {
            return;
        };
        if wanted == LINKED_LIST {
            let Some(index) = first() else { return };
            if !is_loop_counter(ctx, index, node) {
                return;
            }
        } else if runs_once(ctx, node, lp) {
            return;
        }
        if list_class(ctx, recv) != Some(wanted) {
            return;
        }
        // A list declared inside the loop is a new list on every pass: one call per list.
        if let Some(decl) = ctx.declaration_of(recv) {
            if !matches!(decl.kind, VarKind::Field | VarKind::Parameter)
                && decl.statement.start_byte() >= lp.start_byte()
            {
                return;
            }
        }
        let list = ctx.text(recv);
        let msg = match name {
            "get" => format!(
                "Iterate the LinkedList `{list}` instead of calling `get(i)` in a loop: each call walks the list"
            ),
            "remove" => format!(
                "Avoid `remove(0)` on the ArrayList `{list}` in a loop: each call shifts every element; use an ArrayDeque"
            ),
            _ => format!(
                "Avoid `add(0, ..)` on the ArrayList `{list}` in a loop: each call shifts every element; use an ArrayDeque or reverse once"
            ),
        };
        let Some(at) = node.field(Field::Name) else {
            return;
        };
        ctx.emit(at, msg);
    }
}
