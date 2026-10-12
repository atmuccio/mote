//! `java/manual-array-copy` — a loop that only copies elements from one array or collection
//! to another.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["performance", "clutter", "arrays"],
    remediation_minutes: 5,
    description: r#"Detects loops whose only statement copies elements:

* an index loop `for (int i = a; i < b; i++) dst[i + k] = src[i + m];` between two different
  arrays of the same element type;
* a for-each loop `for (T x : source) target.add(x);` adding every element of a collection or
  of an array of objects to a collection.

**Why it matters.** `System.arraycopy`, `Arrays.copyOf`, `addAll` and `Collections.addAll`
state the intent in one call, are checked once instead of per element, and are faster:
`System.arraycopy` is a single memory move, and `addAll` grows the target once instead of
once per element. The hand-written loop hides an easy off-by-one in its bounds.

**How to fix.**

```java
// Noncompliant
for (int i = 0; i < count; i++) {
    copy[i] = values[i];
}
for (String name : names) {
    all.add(name);
}

// Compliant
System.arraycopy(values, 0, copy, 0, count);
all.addAll(names);
```

Not reported: copies within one array (a shifting loop and `System.arraycopy` treat
overlapping ranges differently), arrays of different element types (the loop converts each
element, `System.arraycopy` would throw), loops that also do something else or convert the
element (a cast, unboxing into a primitive loop variable), counters stepping by more than one
or declared before the loop (their final value may be used afterwards), and receivers or
sources whose types are not known."#,
    ..RuleMeta::new(
        "java/manual-array-copy",
        "java",
        "Loop that copies elements one by one",
    )
};

pub struct ManualArrayCopy;

impl<'t> JavaRule<'t> for ManualArrayCopy {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["for_statement", "enhanced_for_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        let stmt = if kind(body) == "block" {
            match util::single_statement(body) {
                Some(s) => s,
                None => return,
            }
        } else {
            body
        };
        if kind(stmt) != "expression_statement" {
            return;
        }
        let Some(expr) = syntax::first_named_child(stmt) else {
            return;
        };
        if kind(node) == "for_statement" {
            check_index_loop(ctx, node, expr);
        } else {
            check_foreach(ctx, node, expr);
        }
    }
}

/// `for (int i = a; i < b; i++) dst[..i..] = src[..i..];`
fn check_index_loop<'t>(ctx: &Ctx<'t>, node: Node<'t>, expr: Node<'t>) {
    if kind(expr) != "assignment_expression" || syntax::operator_str(expr) != "=" {
        return;
    }
    let (Some(l), Some(r)) = (expr.field(Field::Left), expr.field(Field::Right)) else {
        return;
    };
    if kind(l) != "array_access" || kind(r) != "array_access" {
        return;
    }
    let Some(var) = counter(ctx, node) else {
        return;
    };
    let (Some(dst), Some(di), Some(src), Some(si)) = (
        l.field(Field::Array),
        l.field(Field::Index),
        r.field(Field::Array),
        r.field(Field::Index),
    ) else {
        return;
    };
    if !offset_of(ctx, di, var) || !offset_of(ctx, si, var) {
        return;
    }
    if !matches!(kind(dst), "identifier" | "field_access")
        || !matches!(kind(src), "identifier" | "field_access")
    {
        return;
    }
    let (dt, st) = (compact(ctx.text(dst)), compact(ctx.text(src)));
    if dt == st || util::contains_word(&dt, var) || util::contains_word(&st, var) {
        return;
    }
    let (Some(dty), Some(sty)) = (ctx.type_of(dst), ctx.type_of(src)) else {
        return;
    };
    let (Some(de), Some(se)) = (dty.element_type(), sty.element_type()) else {
        return;
    };
    if de != se || !de.is_fully_known() {
        return;
    }
    let kw = node.child(0).unwrap_or(node);
    ctx.emit(
        kw,
        format!("Replace this loop with `System.arraycopy(..)`: it only copies elements of `{st}` into `{dt}`"),
    );
}

/// The counter of `for (int i = a; i < b; i++)` (or `++i`, `i += 1`, `<=`).
fn counter<'t>(ctx: &Ctx<'t>, f: Node<'t>) -> Option<&'t str> {
    let init = f.field(Field::Init)?;
    let var = match kind(init) {
        "local_variable_declaration" => {
            let mut ds = syntax::declarators(init);
            let d = ds.next()?;
            if ds.next().is_some() || d.field(Field::Value).is_none() {
                return None;
            }
            ctx.text(d.field(Field::Name)?)
        }
        // A counter declared outside the loop keeps its final value for later code.
        _ => return None,
    };
    let cond = f.field(Field::Condition)?;
    if kind(cond) != "binary_expression" || !matches!(syntax::operator_str(cond), "<" | "<=") {
        return None;
    }
    let cl = cond.field(Field::Left)?;
    if kind(cl) != "identifier" || ctx.text(cl) != var {
        return None;
    }
    // Exactly one initializer and one update, stepping by one.
    let mut cursor = f.walk();
    if f.children_by_field_name("init", &mut cursor).count() != 1 {
        return None;
    }
    let mut cursor = f.walk();
    let mut updates = f.children_by_field_name("update", &mut cursor);
    let (Some(u), None) = (updates.next(), updates.next()) else {
        return None;
    };
    is_update_of(ctx, u, var).then_some(var)
}

fn is_update_of<'t>(ctx: &Ctx<'t>, n: Node<'t>, var: &str) -> bool {
    match kind(n) {
        "update_expression" => {
            let t = compact(ctx.text(n));
            t == format!("{var}++") || t == format!("++{var}")
        }
        "assignment_expression" => {
            let t = compact(ctx.text(n));
            t == format!("{var}+=1") || t == format!("{var}={var}+1")
        }
        _ => false,
    }
}

/// Whether an index is `i`, `i + e`, `e + i` or `i - e`, with `e` free of `i`.
fn offset_of<'t>(ctx: &Ctx<'t>, idx: Node<'t>, var: &str) -> bool {
    let idx = syntax::strip_parens(idx);
    match kind(idx) {
        "identifier" => ctx.text(idx) == var,
        "binary_expression" => {
            let op = syntax::operator_str(idx);
            let (Some(a), Some(b)) = (idx.field(Field::Left), idx.field(Field::Right)) else {
                return false;
            };
            let is_var = |n: Node<'t>| kind(n) == "identifier" && ctx.text(n) == var;
            let free = |n: Node<'t>| !util::contains_word(ctx.text(n), var);
            match op {
                "+" => (is_var(a) && free(b)) || (is_var(b) && free(a)),
                "-" => is_var(a) && free(b),
                _ => false,
            }
        }
        _ => false,
    }
}

/// `for (T x : source) target.add(x);`
fn check_foreach<'t>(ctx: &Ctx<'t>, node: Node<'t>, expr: Node<'t>) {
    if kind(expr) != "method_invocation" || ctx.call_name(expr) != Some("add") {
        return;
    }
    let (Some(var), Some(source)) = (node.field(Field::Name), node.field(Field::Value)) else {
        return;
    };
    let var = ctx.text(var);
    let mut args = util::args(expr);
    let (Some(arg), None) = (args.next(), args.next()) else {
        return;
    };
    if kind(arg) != "identifier" || ctx.text(arg) != var {
        return;
    }
    let Some(target) = expr.field(Field::Object) else {
        return;
    };
    if !matches!(kind(target), "identifier" | "field_access") {
        return;
    }
    let (tt, st) = (compact(ctx.text(target)), compact(ctx.text(source)));
    if tt == st || util::contains_word(&tt, var) {
        return;
    }
    if !util::is_a(ctx, target, "java.util.Collection") {
        return;
    }
    let Some(src_ty) = ctx.type_of(source) else {
        return;
    };
    let (elem, array) = if src_ty.is_array() {
        (src_ty.element_type().cloned(), true)
    } else if ctx.is_subtype_of(&src_ty, "java.util.Collection") {
        (
            ctx.as_supertype(&src_ty, "java.lang.Iterable")
                .and_then(|t| t.type_arg(0).cloned()),
            false,
        )
    } else {
        return;
    };
    let Some(elem) = elem else {
        return;
    };
    if !elem.is_reference() || !elem.is_fully_known() {
        return;
    }
    // The loop variable must not convert the element.
    let Some(var_ty) = node.field(Field::Type) else {
        return;
    };
    if ctx.text(var_ty) != "var" && ctx.resolve_type(var_ty).as_ref() != Some(&elem) {
        return;
    }
    let fix = if array {
        format!(
            "`Collections.addAll({}, {})`",
            ctx.text(target),
            ctx.text(source)
        )
    } else {
        format!("`{}.addAll({})`", ctx.text(target), ctx.text(source))
    };
    let kw = node.child(0).unwrap_or(node);
    ctx.emit(
        kw,
        format!("Replace this loop with {fix}: it only adds every element"),
    );
}

fn compact(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}
