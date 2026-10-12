//! `java/overwritten-before-read` — two adjacent writes to the same target, the first never read.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["suspicious", "copy-paste"],
    remediation_minutes: 5,
    description: r#"Detects two consecutive statements that write the same target, so the first
value is replaced before anything can read it:

* `map.put(KEY, a); map.put(KEY, b);` — same map, same key (a JDK `Map`);
* `values[i] = a; values[i] = b;` — same array element with a side-effect-free index;
* `this.name = a; this.name = b;` or `x = a; x = b;` — same field or variable.

**Why it matters.** The first write is dead. In practice this is nearly always a copy-paste
slip where the second statement was meant to use a different key, index, or field
(`put("width", w); put("width", h);`), so one value is silently lost and another is never
set.

**How to fix.** Correct the key, index, or target of one of the statements, or delete the
redundant write.

```java
// Noncompliant
size.put("width", width);
size.put("width", height);

// Compliant
size.put("width", width);
size.put("height", height);
```

Not reported: when the second value reads the target (`x = x + 1`, `m.put(k, m.get(k) + 1)`),
when a field or array write is followed by a value that calls methods of this object (they
could read it), volatile fields, compound assignments, and plain variable writes whose
first value has side effects (`line = reader.readLine(); line = reader.readLine();`)."#,
    ..RuleMeta::new(
        "java/overwritten-before-read",
        "java",
        "Value overwritten before it is read",
    )
};

/// The last expression statement seen in each statement list (by the list's node id): each new
/// expression statement is compared with its predecessor in the same list, without iterating
/// every block.
#[derive(Default)]
pub struct OverwrittenBeforeRead<'t> {
    last: FxHashMap<usize, Node<'t>>,
}

/// Whether the source between two statements holds only whitespace and comments.
fn only_trivia(gap: &str) -> bool {
    let mut rest = gap.trim_start();
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix("//") {
            rest = r.find('\n').map_or("", |p| &r[p..]).trim_start();
        } else if let Some(r) = rest.strip_prefix("/*") {
            match r.find("*/") {
                Some(p) => rest = r[p + 2..].trim_start(),
                None => return false,
            }
        } else {
            return false;
        }
    }
    true
}

/// A write performed by an expression statement.
#[derive(Clone, Copy)]
enum Write<'t> {
    /// `recv.put(key, value)`; `at` is the method name.
    Put {
        recv: Node<'t>,
        key: Node<'t>,
        value: Node<'t>,
        at: Node<'t>,
    },
    /// `target = value`.
    Assign { target: Node<'t>, value: Node<'t> },
}

/// A side-effect-free location expression: `x`, `this.x`, `a.b`, literals.
fn simple_path(n: Node<'_>) -> bool {
    let n = syntax::strip_parens(n);
    match kind(n) {
        "identifier" | "this" => true,
        "field_access" => util::fld(n, F::Object).is_some_and(simple_path),
        _ => false,
    }
}

/// A side-effect-free key or index: literals, names, field paths, simple arithmetic on them.
fn stable_expr(n: Node<'_>) -> bool {
    !util::has_side_effects(n) && !util::any_in(n, |_| false, |c| kind(c) == "array_access")
}

fn write_of<'t>(stmt: Node<'t>, src: &str) -> Option<Write<'t>> {
    if kind(stmt) != "expression_statement" {
        return None;
    }
    let e = syntax::first_named_child(stmt)?;
    match kind(e) {
        "method_invocation" => {
            let name = util::fld(e, F::Name)?;
            if syntax::node_text(name, src) != "put" {
                return None;
            }
            let recv = util::fld(e, F::Object)?;
            let mut args = util::args(e);
            let (Some(key), Some(value), None) = (args.next(), args.next(), args.next()) else {
                return None;
            };
            Some(Write::Put {
                recv,
                key,
                value,
                at: name,
            })
        }
        "assignment_expression" => {
            if util::op_str(e) != "=" {
                return None;
            }
            Some(Write::Assign {
                target: util::fld(e, F::Left)?,
                value: util::fld(e, F::Right)?,
            })
        }
        _ => None,
    }
}

/// Same code, ignoring whitespace (the targets compared here are short).
fn same_text(ctx: &Ctx<'_>, a: Node<'_>, b: Node<'_>) -> bool {
    let (ta, tb) = (ctx.text(a), ctx.text(b));
    ta == tb
        || ta
            .bytes()
            .filter(|c| !c.is_ascii_whitespace())
            .eq(tb.bytes().filter(|c| !c.is_ascii_whitespace()))
}

/// Whether `value` calls methods of the current object (unqualified or through `this`),
/// which could read the field or map being written.
fn calls_own_methods(value: Node<'_>) -> bool {
    util::any_in(
        value,
        |n| kind(n) == "lambda_expression",
        |n| match kind(n) {
            "method_invocation" => {
                util::fld(n, F::Object).is_none_or(|o| matches!(kind(o), "this" | "super"))
            }
            "object_creation_expression" => true,
            _ => false,
        },
    )
}

/// The identifier at the root of a target path (`x` of `x`, `this.x`, `x[i]`, `a.x`).
fn base_name<'t>(ctx: &Ctx<'t>, target: Node<'t>) -> Option<&'t str> {
    let t = syntax::strip_parens(target);
    match kind(t) {
        "identifier" => Some(ctx.text(t)),
        "field_access" => util::fld(t, F::Field).map(|f| ctx.text(f)),
        "array_access" => base_name(ctx, util::fld(t, F::Array)?),
        _ => None,
    }
}

/// A literal, an UPPER_CASE constant (possibly qualified), or a local variable / parameter.
fn constant_or_local<'t>(ctx: &Ctx<'t>, key: Node<'t>) -> bool {
    let k = syntax::strip_parens(key);
    match kind(k) {
        "string_literal"
        | "character_literal"
        | "decimal_integer_literal"
        | "hex_integer_literal"
        | "true"
        | "false" => true,
        "identifier" => {
            is_constant_name(ctx.text(k))
                || ctx.declaration_of(k).is_some_and(|d| !d.is_field_like())
        }
        "field_access" => util::fld(k, F::Field).is_some_and(|f| is_constant_name(ctx.text(f))),
        _ => false,
    }
}

fn is_constant_name(s: &str) -> bool {
    s.len() > 1
        && s.starts_with(|c: char| c.is_ascii_uppercase())
        && s.chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

fn w1_value_has_effects(w: Write<'_>) -> bool {
    match w {
        Write::Put { value, .. } | Write::Assign { value, .. } => util::has_side_effects(value),
    }
}

/// Whether evaluating `value` may throw (a call or an object creation), and the statements
/// sit inside a `try` of the same function: then the first value stays visible to the
/// `catch`/`finally` code (`rs = null; rs = open();`) and is not dead.
fn may_throw_into_handler<'t>(ctx: &Ctx<'t>, stmt: Node<'t>, value: Node<'t>) -> bool {
    let may_throw = util::any_in(
        value,
        |n| kind(n) == "lambda_expression",
        |n| matches!(kind(n), "method_invocation" | "object_creation_expression"),
    );
    if !may_throw {
        return false;
    }
    for a in ctx.ancestors(stmt) {
        match kind(a) {
            "try_statement" | "try_with_resources_statement" | "catch_clause" => return true,
            k if syntax::is_function_like(a) || k == "class_body" => return false,
            _ => {}
        }
    }
    false
}

/// Whether the field or variable a target names is `volatile`, or unknown.
fn volatile_or_unknown<'t>(ctx: &Ctx<'t>, target: Node<'t>) -> bool {
    let t = syntax::strip_parens(target);
    match kind(t) {
        "identifier" => match ctx.declaration_of(t) {
            Some(d) => d.kind == VarKind::Field && syntax::has_modifier(d.statement, "volatile"),
            None => true,
        },
        "field_access" => {
            let (Some(obj), Some(field)) = (util::fld(t, F::Object), util::fld(t, F::Field)) else {
                return true;
            };
            if kind(obj) != "this" {
                return true;
            }
            let Some(body) = ctx.enclosing_class(t).and_then(syntax::type_body) else {
                return true;
            };
            match util::find_field(ctx, body, ctx.text(field)) {
                Some((decl, _)) => syntax::has_modifier(decl, "volatile"),
                None => true,
            }
        }
        _ => true,
    }
}

impl<'t> OverwrittenBeforeRead<'t> {
    fn check_pair(&self, ctx: &Ctx<'t>, first: Node<'t>, second: Node<'t>) {
        let src = ctx.source();
        let (Some(w1), Some(w2)) = (write_of(first, src), write_of(second, src)) else {
            return;
        };
        match (w1, w2) {
            (
                Write::Put {
                    recv: r1,
                    key: k1,
                    at,
                    ..
                },
                Write::Put {
                    recv: r2,
                    key: k2,
                    value: v2,
                    ..
                },
            ) => {
                if !simple_path(r1) || !same_text(ctx, r1, r2) || !same_text(ctx, k1, k2) {
                    return;
                }
                if !stable_expr(k1) {
                    return;
                }
                // A key read from a field could be changed by the first value's evaluation.
                if !constant_or_local(ctx, k1) && w1_value_has_effects(w1) {
                    return;
                }
                let Some(name) = base_name(ctx, r1)
                    .or_else(|| (kind(syntax::strip_parens(r1)) == "this").then_some("this"))
                else {
                    return;
                };
                if util::mentions(ctx, v2, name)
                    || calls_own_methods(v2)
                    || may_throw_into_handler(ctx, first, v2)
                {
                    return;
                }
                // Only JDK maps are known to simply replace the value.
                let is_jdk_map = ctx.type_of(r1).is_some_and(|t| {
                    t.fqn().is_some_and(|f| f.starts_with("java.util."))
                        && ctx.is_subtype_of(&t, "java.util.Map")
                });
                if !is_jdk_map {
                    return;
                }
                let key = ctx.text(k1);
                let key = if key.len() > 40 { "the same key" } else { key };
                ctx.emit(
                    at,
                    format!("This put() is overwritten by the next statement, which puts `{key}` again before the value is read"),
                )
                .secondary(second, "overwritten here");
            }
            (
                Write::Assign {
                    target: t1,
                    value: v1,
                },
                Write::Assign {
                    target: t2,
                    value: v2,
                },
            ) => {
                let t1s = syntax::strip_parens(t1);
                let is_array = kind(t1s) == "array_access";
                let path_ok = match kind(t1s) {
                    "identifier" => true,
                    "field_access" => util::fld(t1s, F::Object).is_some_and(|o| kind(o) == "this"),
                    "array_access" => {
                        util::fld(t1s, F::Array).is_some_and(simple_path)
                            && util::fld(t1s, F::Index).is_some_and(stable_expr)
                    }
                    _ => false,
                };
                if !path_ok || !same_text(ctx, t1, t2) {
                    return;
                }
                let Some(name) = base_name(ctx, t1) else {
                    return;
                };
                if util::mentions(ctx, v2, name) {
                    return;
                }
                // Array elements and fields may be read by calls; plain variables may be written
                // twice on purpose to run the first value's side effects.
                let is_local = kind(t1s) == "identifier"
                    && ctx.declaration_of(t1s).is_some_and(|d| !d.is_field_like());
                if (!is_local && calls_own_methods(v2)) || may_throw_into_handler(ctx, first, v2) {
                    return;
                }
                // A first value with side effects may be evaluated on purpose (or may even
                // change the index or field path of the second write).
                if util::has_side_effects(v1) {
                    return;
                }
                if !is_array && volatile_or_unknown(ctx, t1s) {
                    return;
                }
                let shown = ctx.text(t1);
                let shown = if shown.len() > 40 {
                    "this target"
                } else {
                    shown
                };
                ctx.emit(
                    t1,
                    format!("The value written to `{shown}` here is overwritten by the next statement before it is read"),
                )
                .secondary(second, "overwritten here");
            }
            _ => {}
        }
    }
}

/// A cheap textual key for a write statement: the source text from the start of the statement
/// to the end of the written target (`x`, `this.f`, `a[i]`) or of the `put` key
/// (`map.put(K`). Two adjacent statements can only overwrite each other when keys match.
fn quick_key<'t>(ctx: &Ctx<'t>, stmt: Node<'t>) -> Option<&'t str> {
    if kind(stmt) != "expression_statement" {
        return None;
    }
    let e = util::first_child(stmt)?;
    let end = match kind(e) {
        "assignment_expression" => util::fld(e, F::Left)?.end_byte(),
        "method_invocation" => {
            if util::name_text(ctx, e) != "put" {
                return None;
            }
            util::first_arg(e)?.end_byte()
        }
        _ => return None,
    };
    ctx.source().get(e.start_byte()..end)
}

impl<'t> JavaRule<'t> for OverwrittenBeforeRead<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["expression_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(parent) = ctx.parent(node) else {
            return;
        };
        if !matches!(
            kind(parent),
            "block" | "constructor_body" | "switch_block_statement_group"
        ) {
            return;
        }
        let Some(prev) = self.last.insert(parent.id(), node) else {
            return;
        };
        let src = ctx.source();
        // Equal keys imply equal leading bytes: compare a few bytes before touching the tree.
        let (a, b) = (prev.start_byte(), node.start_byte());
        let n = 6.min(prev.end_byte() - a).min(node.end_byte() - b);
        if src.as_bytes().get(a..a + n) != src.as_bytes().get(b..b + n) {
            return;
        }
        // Adjacent: nothing but whitespace and comments in between.
        if !src.get(prev.end_byte()..b).is_some_and(only_trivia) {
            return;
        }
        let (pk, k) = (quick_key(ctx, prev), quick_key(ctx, node));
        if pk.is_some() && pk == k {
            self.check_pair(ctx, prev, node);
        }
    }
}
