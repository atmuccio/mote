//! Helpers shared by the batch-7 rules: a per-file text gate, bounded subtree visits, call
//! arguments, literal and receiver tests. Nothing here walks the whole tree.

use crate::prelude::*;

/// Per-file "does the source text contain `needle`?", computed on first use. Lets a rule skip
/// resolver work in the (many) files that cannot contain what it looks for.
#[derive(Default)]
pub(super) struct Gate(Option<bool>);

impl Gate {
    pub(super) fn open(&mut self, ctx: &Ctx<'_>, needle: &str) -> bool {
        *self.0.get_or_insert_with(|| ctx.source().contains(needle))
    }

    /// Like [`Gate::open`], true when any of the needles occurs.
    pub(super) fn open_any(&mut self, ctx: &Ctx<'_>, needles: &[&str]) -> bool {
        *self
            .0
            .get_or_insert_with(|| needles.iter().any(|n| ctx.source().contains(n)))
    }
}

/// What to do after visiting a node in [`visit`].
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Walk {
    /// Descend into the node's children.
    Continue,
    /// Don't look inside this node.
    Skip,
    /// End the whole visit.
    Stop,
}

/// Pre-order visit of the **named** descendants of `root` (not `root` itself), in source order.
/// Returns `true` when the callback stopped the visit. Linear in the size of the subtree.
pub(super) fn visit<'t>(root: Node<'t>, mut f: impl FnMut(Node<'t>) -> Walk) -> bool {
    let mut cursor = root.walk();
    if !cursor.goto_first_child() {
        return false;
    }
    loop {
        let n = cursor.node();
        let action = if n.is_named() { f(n) } else { Walk::Skip };
        match action {
            Walk::Stop => return true,
            Walk::Continue if cursor.goto_first_child() => continue,
            _ => {}
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() || cursor.node().id() == root.id() {
                return false;
            }
        }
    }
}

/// Whether code inside `n` runs in another function or object (lambda, class body, local or
/// anonymous class): visits of one method body stop there.
pub(super) fn is_nested_scope(n: Node<'_>) -> bool {
    matches!(
        kind(n),
        "lambda_expression"
            | "class_body"
            | "class_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "record_declaration"
            | "annotation_type_declaration"
    )
}

/// Whether any named node of the subtree (the root excluded) satisfies `pred`, not entering
/// lambdas and class bodies.
pub(super) fn any_in_scope<'t>(root: Node<'t>, mut pred: impl FnMut(Node<'t>) -> bool) -> bool {
    visit(root, |n| {
        if is_nested_scope(n) {
            Walk::Skip
        } else if pred(n) {
            Walk::Stop
        } else {
            Walk::Continue
        }
    })
}

/// The first named node of the subtree (the root excluded, lambdas and class bodies not
/// entered) that satisfies `pred`.
pub(super) fn find_in_scope<'t>(
    root: Node<'t>,
    mut pred: impl FnMut(Node<'t>) -> bool,
) -> Option<Node<'t>> {
    let mut found = None;
    visit(root, |n| {
        if is_nested_scope(n) {
            Walk::Skip
        } else if pred(n) {
            found = Some(n);
            Walk::Stop
        } else {
            Walk::Continue
        }
    });
    found
}

/// Whether an expression may have an effect or yield a fresh value each time it is evaluated.
pub(super) fn has_side_effects(node: Node<'_>) -> bool {
    let bad = |n: Node<'_>| {
        matches!(
            kind(n),
            "method_invocation"
                | "object_creation_expression"
                | "array_creation_expression"
                | "assignment_expression"
                | "update_expression"
                | "lambda_expression"
                | "method_reference"
                | "switch_expression"
                | "ERROR"
        )
    };
    bad(node) || visit(node, |n| if bad(n) { Walk::Stop } else { Walk::Continue })
}

/// The argument expressions of a call or object creation (comments skipped).
pub(super) fn args(call: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    call.field(Field::Arguments)
        .into_iter()
        .flat_map(named_children)
}

/// Number of arguments of a call or object creation.
pub(super) fn arg_count(call: Node<'_>) -> usize {
    args(call).count()
}

/// The `i`-th argument of a call or object creation.
pub(super) fn arg(call: Node<'_>, i: usize) -> Option<Node<'_>> {
    args(call).nth(i)
}

/// The value of an integer literal (a leading minus honoured, parentheses skipped).
pub(super) fn int_value(ctx: &Ctx<'_>, node: Node<'_>) -> Option<i64> {
    syntax::int_literal_value(node, ctx.source())
}

/// Whether `expr` names the type `fqn` (the receiver of a static call like `Math.abs`).
pub(super) fn names_type<'t>(ctx: &Ctx<'t>, expr: Node<'t>, fqn: &str) -> bool {
    let simple = fqn.rsplit('.').next().unwrap_or(fqn);
    let text = ctx.text(expr);
    if text != simple && text != fqn {
        return false;
    }
    ctx.type_name_of(expr).as_deref() == Some(fqn)
}

/// Whether the receiver of `call` is provably of type `fqn` or a subtype of any of `fqns`.
pub(super) fn receiver_is<'t>(ctx: &Ctx<'t>, call: Node<'t>, fqns: &[&str]) -> bool {
    let Some(obj) = call.field(Field::Object) else {
        return false;
    };
    if matches!(kind(obj), "this" | "super") {
        return false;
    }
    let Some(t) = ctx.type_of(obj) else {
        return false;
    };
    fqns.iter().any(|f| ctx.is_subtype_of(&t, f))
}

/// Whether `expr` is a plain reference: `x`, `this.x`, `a.b.c` (no calls, no indexing).
pub(super) fn is_plain_ref(expr: Node<'_>) -> bool {
    let expr = syntax::strip_parens(expr);
    match kind(expr) {
        "identifier" => true,
        "field_access" => {
            let (Some(o), Some(f)) = (expr.field(Field::Object), expr.field(Field::Field)) else {
                return false;
            };
            kind(f) == "identifier" && (kind(o) == "this" || is_plain_ref(o))
        }
        _ => false,
    }
}

/// The variable or field a target names: `x`, or `f` of `this.f`.
pub(super) fn var_name<'t>(ctx: &Ctx<'t>, expr: Node<'t>) -> Option<&'t str> {
    let expr = syntax::strip_parens(expr);
    match kind(expr) {
        "identifier" => Some(ctx.text(expr)),
        "field_access" => match (expr.field(Field::Object), expr.field(Field::Field)) {
            (Some(o), Some(f)) if kind(o) == "this" && kind(f) == "identifier" => Some(ctx.text(f)),
            _ => None,
        },
        _ => None,
    }
}

/// The last segment of a dotted name (`Calendar.MONTH` → `MONTH`).
pub(super) fn last_segment(text: &str) -> &str {
    text.rsplit('.').next().unwrap_or(text).trim()
}

/// Whether `name` is a zero-parameter method declared directly in `body` (a class body).
pub(super) fn declares_method<'t>(
    ctx: &Ctx<'t>,
    body: Node<'t>,
    name: &str,
    params: usize,
) -> bool {
    ctx.members(body).any(|m| {
        kind(m) == "method_declaration"
            && syntax::name_of(m, ctx.source()) == name
            && syntax::parameters(m).count() == params
    })
}

/// Whether `expr` names `java.lang.Math` or `java.lang.StrictMath` (the latter is not in the
/// JDK table: its simple name is accepted unless it resolves to another type).
pub(super) fn names_math<'t>(ctx: &Ctx<'t>, expr: Node<'t>) -> bool {
    if names_type(ctx, expr, "java.lang.Math") {
        return true;
    }
    let text = ctx.text(expr);
    (text == "StrictMath" || text == "java.lang.StrictMath")
        && ctx
            .type_name_of(expr)
            .is_none_or(|f| f == "java.lang.StrictMath")
}
