//! `java/index-loop-could-be-foreach` — `for (int i = 0; i < a.length; i++)` that only reads
//! `a[i]`.

use super::util::{self, Walk, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["idiom", "readability"],
    remediation_minutes: 2,
    description: r#"Detects counting loops over an array or a `List` —
`for (int i = 0; i < a.length; i++)` or `for (int i = 0; i < list.size(); i++)` — whose body
uses the index only to read the current element (`a[i]`, `list.get(i)`).

**Why it matters.** The enhanced `for` says the same thing with less machinery: no index to
get wrong (`<=`, a wrong bound, a stray `i++` in the body), no repeated `list.get(i)`, and the
element gets a name. On linked lists it is also much faster than `get(i)`.

**How to fix.** Iterate over the elements directly.

```java
// Noncompliant
for (int i = 0; i < names.size(); i++) {
    print(names.get(i));
}

// Compliant
for (String name : names) {
    print(name);
}
```

Also detects the iterator form, `for (Iterator<T> it = items.iterator(); it.hasNext();)`, when
the body calls `it.next()` exactly once on every pass and uses the iterator for nothing else.

Only the plain patterns are reported. Counting loops: an `int` index starting at `0`, a `<`
bound on the same array's `length` or list's `size()`, a `++` step, and a body in which every
use of the index is `a[i]` / `list.get(i)` on that same variable, which is itself used for
nothing else. Writes to the element (`a[i] = …`) keep the loop as it is. Lists must be known
`java.util.List`s. Iterator loops: no update clause, a collection known to be `Iterable` (a
variable declared in this file with a JDK collection type, a map view such as `entrySet()`),
and a single `it.next()` that is not behind a condition, a `&&`/`||`, an inner loop, or a
lambda, with no `break`, `continue`, `return` or `throw` before it. Loops that call
`it.remove()` or pass the iterator on need it and are left alone."#,
    ..RuleMeta::new(
        "java/index-loop-could-be-foreach",
        "java",
        "Index loop could be an enhanced for",
    )
};

pub struct IndexLoopCouldBeForeach;

/// Whether `n` is `<it>.<method>()` with no arguments.
fn is_iterator_call<'t>(ctx: &Ctx<'t>, n: Node<'t>, it: &str, method: &str) -> bool {
    kind(n) == "method_invocation"
        && util::call_name(ctx, n) == method
        && util::arg_count(n) == 0
        && util::fld(n, F.object).is_some_and(|o| kind(o) == "identifier" && ctx.text(o) == it)
}

/// Whether `call` runs exactly once per execution of `stmt` (its enclosing top-level statement):
/// not behind a condition, a short-circuit operand, a loop, a lambda, or a handler.
fn runs_unconditionally<'t>(ctx: &Ctx<'t>, call: Node<'t>, stmt: Node<'t>) -> bool {
    let mut child = call;
    for a in ctx.ancestors(call) {
        let ok = match kind(a) {
            "if_statement" | "ternary_expression" | "switch_expression" => {
                util::fld(a, F.condition).is_some_and(|c| c.id() == child.id())
            }
            "binary_expression" => {
                !matches!(syntax::operator_str(a), "&&" | "||")
                    || util::fld(a, F.left).is_some_and(|l| l.id() == child.id())
            }
            "while_statement"
            | "do_statement"
            | "for_statement"
            | "enhanced_for_statement"
            | "lambda_expression"
            | "class_body"
            | "catch_clause"
            | "finally_clause"
            | "switch_block"
            | "switch_block_statement_group"
            | "switch_rule" => false,
            _ => true,
        };
        if !ok {
            return false;
        }
        if a.id() == stmt.id() {
            return true;
        }
        child = a;
    }
    false
}

/// `for (Iterator<T> it = items.iterator(); it.hasNext();) { T x = it.next(); … }`: the
/// `items` expression, when the loop could be an enhanced `for` over it.
fn iterator_loop<'t>(ctx: &Ctx<'t>, node: Node<'t>, init: Node<'t>) -> Option<Node<'t>> {
    let ty = util::fld(init, F.ty)?;
    let base = util::type_base(ty);
    if !matches!(ctx.text(base), "Iterator" | "java.util.Iterator" | "var") {
        return None;
    }
    let mut decls = syntax::declarators(init);
    let (Some(decl), None) = (decls.next(), decls.next()) else {
        return None;
    };
    let it = ctx.text(util::fld(decl, F.name)?);
    let value = syntax::strip_parens(util::fld(decl, F.value)?);
    if kind(value) != "method_invocation"
        || util::call_name(ctx, value) != "iterator"
        || util::arg_count(value) != 0
    {
        return None;
    }
    let items = util::fld(value, F.object)?;
    if !is_iterator_call(ctx, util::fld(node, F.condition)?, it, "hasNext") {
        return None;
    }
    let mut cursor = node.walk();
    if node
        .children_by_field_name("update", &mut cursor)
        .next()
        .is_some()
    {
        return None;
    }
    let body = util::fld(node, F.body)?;
    if kind(body) != "block" {
        return None;
    }
    // Every mention of the iterator is the one `it.next()`.
    let mut next_call: Option<Node<'t>> = None;
    let mut other_use = false;
    util::visit(body, |n| {
        if kind(n) == "identifier" && ctx.text(n) == it {
            match ctx.parent(n) {
                Some(p) if next_call.is_none() && is_iterator_call(ctx, p, it, "next") => {
                    next_call = Some(p);
                }
                _ => {
                    other_use = true;
                    return Walk::Stop;
                }
            }
        }
        Walk::Continue
    });
    let call = next_call?;
    if other_use {
        return None;
    }
    // The top-level statement holding the call, and nothing before it that leaves the pass.
    let stmt = named_children(body)
        .find(|s| s.start_byte() <= call.start_byte() && call.end_byte() <= s.end_byte())?;
    if !runs_unconditionally(ctx, call, stmt) {
        return None;
    }
    let leaves = named_children(body)
        .take_while(|s| s.id() != stmt.id())
        .any(|s| {
            util::visit(s, |n| match kind(n) {
                "break_statement" | "continue_statement" | "return_statement"
                | "throw_statement" | "yield_statement" => Walk::Stop,
                _ => Walk::Continue,
            }) || matches!(
                kind(s),
                "break_statement" | "continue_statement" | "return_statement" | "throw_statement"
            )
        });
    if leaves {
        return None;
    }
    let t = ctx.type_of(items)?;
    ctx.is_subtype_of(&t, "java.lang.Iterable").then_some(items)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    Array,
    List,
}

/// `i++`, `++i`, or `i += 1`.
fn is_increment(ctx: &Ctx<'_>, update: Node<'_>, index: &str) -> bool {
    match kind(update) {
        "update_expression" => {
            syntax::children(update).any(|c| kind(c) == "++")
                && syntax::first_named_child(update)
                    .is_some_and(|v| kind(v) == "identifier" && ctx.text(v) == index)
        }
        "assignment_expression" => {
            syntax::operator_str(update) == "+="
                && util::fld(update, F.left)
                    .is_some_and(|l| kind(l) == "identifier" && ctx.text(l) == index)
                && util::fld(update, F.right)
                    .is_some_and(|r| syntax::is_int_literal(r, ctx.source(), 1))
        }
        _ => false,
    }
}

impl<'t> JavaRule<'t> for IndexLoopCouldBeForeach {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["for_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let src = ctx.source();
        // init: `int i = 0`
        let Some(init) = util::fld(node, F.init) else {
            return;
        };
        if kind(init) != "local_variable_declaration" {
            return;
        }
        if util::fld(init, F.ty).is_none_or(|t| ctx.text(t) != "int") {
            if let Some(items) = iterator_loop(ctx, node, init) {
                let kw = syntax::children(node).next().unwrap_or(node);
                ctx.emit(
                    kw,
                    format!(
                        "Replace this iterator loop with an enhanced `for` over `{}`",
                        util::shorten(ctx.text(items), 40)
                    ),
                );
            }
            return;
        }
        let mut decls = syntax::declarators(init);
        let (Some(decl), None) = (decls.next(), decls.next()) else {
            return;
        };
        let (Some(index_node), Some(start)) = (util::fld(decl, F.name), util::fld(decl, F.value))
        else {
            return;
        };
        if !syntax::is_int_literal(start, src, 0) {
            return;
        }
        let index = ctx.text(index_node);
        // condition: `i < X.length` / `i < X.size()`
        let Some(cond) = util::fld(node, F.condition) else {
            return;
        };
        if kind(cond) != "binary_expression" || syntax::operator_str(cond) != "<" {
            return;
        }
        let (Some(left), Some(right)) = (util::fld(cond, F.left), util::fld(cond, F.right)) else {
            return;
        };
        if kind(left) != "identifier" || ctx.text(left) != index {
            return;
        }
        let (shape, target) = match kind(right) {
            "field_access" => {
                let (Some(obj), Some(field)) =
                    (util::fld(right, F.object), util::fld(right, F.field))
                else {
                    return;
                };
                if ctx.text(field) != "length" {
                    return;
                }
                (Shape::Array, obj)
            }
            "method_invocation" => {
                if util::call_name(ctx, right) != "size" || util::arg_count(right) != 0 {
                    return;
                }
                let Some(obj) = util::fld(right, F.object) else {
                    return;
                };
                (Shape::List, obj)
            }
            _ => return,
        };
        if kind(target) != "identifier" {
            return;
        }
        let target_name = ctx.text(target);
        if target_name == index {
            return;
        }
        // update: exactly one `i++`
        let mut update_count = 0;
        let mut update_ok = true;
        let mut cursor = node.walk();
        for u in node.children_by_field_name("update", &mut cursor) {
            update_count += 1;
            update_ok &= is_increment(ctx, u, index);
        }
        if update_count != 1 || !update_ok {
            return;
        }
        let Some(body) = util::fld(node, F.body) else {
            return;
        };
        // Every use of `i` and of the target is one element read.
        let mut reads = 0usize;
        let mut index_uses = 0usize;
        let mut target_uses = 0usize;
        let mut bad = false;
        util::visit(body, |n| {
            if kind(n) != "identifier" {
                return Walk::Continue;
            }
            let t = ctx.text(n);
            if t == index {
                index_uses += 1;
            } else if t == target_name {
                target_uses += 1;
            }
            Walk::Continue
        });
        util::visit(body, |n| {
            match (shape, kind(n)) {
                (Shape::Array, "array_access") => {
                    let (Some(arr), Some(idx)) = (util::fld(n, F.array), util::fld(n, F.index))
                    else {
                        return Walk::Continue;
                    };
                    if kind(arr) == "identifier"
                        && ctx.text(arr) == target_name
                        && kind(idx) == "identifier"
                        && ctx.text(idx) == index
                    {
                        // Not a write target.
                        match ctx.parent(n) {
                            Some(p)
                                if (kind(p) == "assignment_expression"
                                    && util::fld(p, F.left).is_some_and(|l| l.id() == n.id()))
                                    || kind(p) == "update_expression" =>
                            {
                                bad = true;
                                return Walk::Stop;
                            }
                            _ => {}
                        }
                        reads += 1;
                    }
                }
                (Shape::List, "method_invocation")
                    if util::call_name(ctx, n) == "get"
                        && util::arg_count(n) == 1
                        && util::fld(n, F.object).is_some_and(|o| {
                            kind(o) == "identifier" && ctx.text(o) == target_name
                        })
                        && util::arg(n, 0)
                            .is_some_and(|a| kind(a) == "identifier" && ctx.text(a) == index) =>
                {
                    reads += 1;
                }
                _ => {}
            }
            Walk::Continue
        });
        if bad || reads == 0 || index_uses != reads || target_uses != reads {
            return;
        }
        // The type proves the shape: arrays need no check (`a[i]` compiles only on arrays);
        // `get(i)`/`size()` must belong to a `java.util.List`.
        if shape == Shape::List {
            let Some(t) = ctx.type_of(target) else {
                return;
            };
            if !ctx.is_subtype_of(&t, "java.util.List") {
                return;
            }
        }
        let kw = syntax::children(node).next().unwrap_or(node);
        ctx.emit(
            kw,
            format!("Replace this index loop with an enhanced `for` over `{target_name}`"),
        );
    }
}
