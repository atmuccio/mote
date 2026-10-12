//! `java/boxed-boolean-condition` — a `Boolean` object used directly as a condition.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["null-safety", "boxing"],
    remediation_minutes: 2,
    description: r#"Detects an expression of type `java.lang.Boolean` (the wrapper, not the
primitive) used directly as the condition of an `if`, `while`, `do`, `for`, or `?:`, or as
the operand of `!`.

**Why it matters.** The condition auto-unboxes the wrapper with `booleanValue()`. A
`Boolean` can be `null` — a missing map entry, an unset field, a nullable database
column — and then the condition throws `NullPointerException` instead of taking a branch.

**How to fix.** Decide what `null` means and say so explicitly: `Boolean.TRUE.equals(flag)`
(null counts as false), `!Boolean.FALSE.equals(flag)` (null counts as true), or use a
primitive `boolean` where `null` is not a valid value.

```java
// Noncompliant
Boolean enabled = settings.get("enabled");
if (enabled) { start(); }

// Compliant
if (Boolean.TRUE.equals(enabled)) { start(); }
```

Not reported: `Boolean.TRUE`, `Boolean.FALSE`, `Boolean.valueOf(..)`, and values unwrapped
with a non-null fallback (`opt.orElse(false)`, `defaultIfNull(x, Boolean.FALSE)`,
`requireNonNullElse(x, false)`), which are never null; local variables initialized with a
non-null value, or declared without one and only ever assigned non-null values; pattern variables (`o instanceof Boolean
b`), and variables that the same method compares with `null`. Only expressions whose type
the resolver proves to be `Boolean` are reported, and only when the variable, field, or
method involved is declared in this file with a type mentioning `Boolean`."#,
    ..RuleMeta::new(
        "java/boxed-boolean-condition",
        "java",
        "Boxed Boolean used as a condition",
    )
};

/// Names declared with a type that mentions `Boolean` (variables, fields, parameters) and
/// methods declared to return one, collected syntactically during the walk; conditions are
/// recorded as candidates and resolved in `finish` only when one of their names is in the set.
#[derive(Default)]
pub struct BoxedBooleanCondition<'t> {
    /// Whether the file mentions `Boolean` at all.
    boolean_in_file: util::Gate,
    boolean_names: FxHashSet<&'t str>,
    /// (the statement or expression owning the condition, the condition)
    candidates: Vec<(Node<'t>, Node<'t>)>,
}

/// Whether `Boolean` occurs as a type name rather than only in `Boolean.TRUE` /
/// `Boolean.parseBoolean(..)` or inside a longer name (`AtomicBoolean`).
fn mentions_boolean_type(text: &str) -> bool {
    let bytes = text.as_bytes();
    text.match_indices("Boolean").any(|(i, _)| {
        let before = i.checked_sub(1).and_then(|j| bytes.get(j)).copied();
        let after = bytes.get(i + "Boolean".len()).copied();
        !before.is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'$')
            && !after.is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'.')
    })
}

/// The names a candidate condition depends on: the variable, the method, the receiver root.
fn condition_names<'t>(ctx: &Ctx<'t>, cond: Node<'t>, out: &mut Vec<&'t str>) {
    let mut cur = cond;
    for _ in 0..32 {
        cur = syntax::strip_parens(cur);
        match kind(cur) {
            "identifier" => {
                out.push(ctx.text(cur));
                return;
            }
            "field_access" => {
                if let Some(f) = util::fld(cur, F::Field) {
                    out.push(ctx.text(f));
                }
                match util::fld(cur, F::Object) {
                    Some(o) => cur = o,
                    None => return,
                }
            }
            "method_invocation" => {
                if let Some(n) = util::fld(cur, F::Name) {
                    out.push(ctx.text(n));
                }
                match util::fld(cur, F::Object) {
                    Some(o) => cur = o,
                    None => return,
                }
            }
            "array_access" => match util::fld(cur, F::Array) {
                Some(a) => cur = a,
                None => return,
            },
            _ => return,
        }
    }
}

/// Syntactic shapes that can have a `Boolean` type (not comparisons, literals, `&&` …).
fn candidate(n: Node<'_>) -> bool {
    matches!(
        kind(n),
        "identifier" | "field_access" | "method_invocation" | "array_access"
    )
}

/// Whether `expr` is `Boolean.TRUE`, `Boolean.FALSE`, `Boolean.valueOf(..)`, a boolean
/// literal, or a value unwrapped with such a fallback (`opt.orElse(false)`,
/// `defaultIfNull(x, Boolean.FALSE)`, `requireNonNullElse(x, true)`), possibly cast.
fn never_null<'t>(ctx: &Ctx<'t>, expr: Node<'t>) -> bool {
    never_null_d(ctx, expr, 0)
}

fn never_null_d<'t>(ctx: &Ctx<'t>, expr: Node<'t>, depth: u32) -> bool {
    let mut expr = syntax::strip_parens(expr);
    if kind(expr) == "cast_expression" {
        match util::fld(expr, F::Value) {
            Some(v) => expr = syntax::strip_parens(v),
            None => return false,
        }
    }
    if depth > 4 {
        return false;
    }
    if kind(expr) == "method_invocation" {
        // The fallback argument of a null-defaulting call.
        let fallback = match util::name_text(ctx, expr) {
            "orElse" => {
                let mut args = util::args(expr);
                match (args.next(), args.next()) {
                    (Some(a), None) => Some(a),
                    _ => None,
                }
            }
            "defaultIfNull" | "requireNonNullElse" => util::args(expr).nth(1),
            _ => None,
        };
        if fallback.is_some_and(|f| never_null_d(ctx, f, depth + 1)) {
            return true;
        }
    }
    match kind(expr) {
        "field_access" => {
            util::fld(expr, F::Field).is_some_and(|f| matches!(ctx.text(f), "TRUE" | "FALSE"))
                && util::fld(expr, F::Object)
                    .is_some_and(|o| util::names_type(ctx, o, "java.lang.Boolean"))
        }
        "method_invocation" => {
            util::name_text(ctx, expr) == "valueOf"
                && util::fld(expr, F::Object)
                    .is_some_and(|o| util::names_type(ctx, o, "java.lang.Boolean"))
        }
        "true" | "false" => true,
        _ => false,
    }
}

/// Whether the enclosing function compares `name` with `null` somewhere.
fn null_checked<'t>(ctx: &Ctx<'t>, at: Node<'t>, name: &str) -> bool {
    let Some(func) = ctx.enclosing_function(at) else {
        return false;
    };
    util::any_in(
        func,
        |_| false,
        |n| match kind(n) {
            "binary_expression" => {
                matches!(util::op_str(n), "==" | "!=")
                    && [F::Left, F::Right]
                        .iter()
                        .any(|f| util::fld(n, *f).is_some_and(|s| syntax::is_null_literal(s)))
                    && [F::Left, F::Right].iter().any(|f| {
                        util::fld(n, *f).is_some_and(|s| {
                            let s = syntax::strip_parens(s);
                            let t = ctx.text(s);
                            t == name || t.strip_prefix("this.") == Some(name)
                        })
                    })
            }
            "method_invocation" => {
                matches!(
                    util::name_text(ctx, n),
                    "isNull"
                        | "nonNull"
                        | "requireNonNull"
                        | "requireNonNullElse"
                        | "isTrue"
                        | "isFalse"
                        | "notNull"
                ) && util::args(n).any(|a| ctx.text(a) == name)
            }
            _ => false,
        },
    )
}

/// Whether the enclosing function assigns `name` at least once and only with values that are
/// never null (`never_null` or of primitive `boolean` type).
fn only_non_null_assignments<'t>(ctx: &Ctx<'t>, at: Node<'t>, name: &str) -> bool {
    let Some(func) = ctx.enclosing_function(at) else {
        return false;
    };
    let mut assigned = false;
    let nullable = util::any_in(
        func,
        |_| false,
        |n| {
            if kind(n) != "assignment_expression"
                || util::fld(n, F::Left).is_none_or(|l| ctx.text(l) != name)
            {
                return false;
            }
            assigned = true;
            util::op_str(n) != "="
                || !util::fld(n, F::Right).is_some_and(|v| {
                    never_null(ctx, v) || ctx.type_of(v).is_some_and(|vt| vt.is_boolean())
                })
        },
    );
    assigned && !nullable
}

impl<'t> BoxedBooleanCondition<'t> {
    /// Declarations whose type mentions `Boolean`: remember the declared names.
    fn collect_declaration(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(ty) = util::fld(node, F::Type) else {
            return;
        };
        if !mentions_boolean_type(ctx.text(ty)) {
            return;
        }
        match kind(node) {
            "local_variable_declaration" | "field_declaration" => {
                for d in syntax::declarators(node) {
                    if let Some(n) = util::fld(d, F::Name) {
                        self.boolean_names.insert(ctx.text(n));
                    }
                }
            }
            _ => {
                if let Some(n) = util::fld(node, F::Name) {
                    self.boolean_names.insert(ctx.text(n));
                }
            }
        }
    }

    fn check_condition(&self, ctx: &Ctx<'t>, owner: Node<'t>, cond: Node<'t>) {
        if never_null(ctx, cond) {
            return;
        }
        let Some(t) = ctx.type_of(cond) else {
            return;
        };
        if !t.is_class("java.lang.Boolean") {
            return;
        }
        // Variables: skip non-null locals and anything the method null-checks.
        let var_name = match kind(cond) {
            "identifier" => Some(cond),
            "field_access" => util::fld(cond, F::Object)
                .filter(|o| kind(*o) == "this")
                .and(util::fld(cond, F::Field)),
            _ => None,
        };
        if let Some(var) = var_name {
            let name = ctx.text(var);
            if kind(cond) == "identifier" {
                if let Some(d) = ctx.declaration_of(cond) {
                    // `o instanceof Boolean b` binds a non-null value.
                    if d.kind == VarKind::PatternVariable {
                        return;
                    }
                    if matches!(d.kind, VarKind::Local | VarKind::Resource)
                        && d.value.is_some_and(|v| {
                            never_null(ctx, v) || ctx.type_of(v).is_some_and(|vt| vt.is_boolean())
                        })
                    {
                        return;
                    }
                    // Declared without a value: definite assignment guarantees one of its
                    // assignments ran, so it is non-null when all of them are.
                    if d.kind == VarKind::Local
                        && d.value.is_none()
                        && only_non_null_assignments(ctx, owner, name)
                    {
                        return;
                    }
                }
            }
            if null_checked(ctx, owner, name) {
                return;
            }
        }
        let shown = ctx.text(cond);
        let shown = if shown.len() > 40 {
            "this Boolean"
        } else {
            shown
        };
        ctx.emit(
            cond,
            format!("Compare `{shown}` explicitly (e.g. Boolean.TRUE.equals(..)): unboxing a null Boolean throws NullPointerException"),
        );
    }
}

impl<'t> JavaRule<'t> for BoxedBooleanCondition<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            // conditions
            "if_statement",
            "while_statement",
            "do_statement",
            "for_statement",
            "ternary_expression",
            "unary_expression",
            // declarations that may introduce a `Boolean`
            "local_variable_declaration",
            "field_declaration",
            "formal_parameter",
            "method_declaration",
            "enhanced_for_statement",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !self.boolean_in_file.open(ctx, "Boolean") {
            return;
        }
        let cond = match kind(node) {
            "unary_expression" => {
                if util::op_str(node) != "!" {
                    return;
                }
                util::fld(node, F::Operand)
            }
            "local_variable_declaration"
            | "field_declaration"
            | "formal_parameter"
            | "method_declaration"
            | "enhanced_for_statement" => {
                self.collect_declaration(node, ctx);
                return;
            }
            _ => util::fld(node, F::Condition),
        };
        let Some(cond) = cond else {
            return;
        };
        let cond = syntax::strip_parens(cond);
        if candidate(cond) {
            self.candidates.push((node, cond));
        }
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        if self.boolean_names.is_empty() {
            return;
        }
        let candidates = std::mem::take(&mut self.candidates);
        let mut names = Vec::new();
        // Bounded resolver work per file.
        let mut budget = 2048u32;
        for (owner, cond) in candidates {
            names.clear();
            condition_names(ctx, cond, &mut names);
            if !names.iter().any(|n| self.boolean_names.contains(n)) {
                continue;
            }
            if budget == 0 {
                break;
            }
            budget -= 1;
            self.check_condition(ctx, owner, cond);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::mentions_boolean_type;

    #[test]
    fn finds_boolean_type_uses() {
        assert!(mentions_boolean_type("Boolean"));
        assert!(mentions_boolean_type("Map<String, Boolean>"));
        assert!(mentions_boolean_type("Boolean[]"));
        assert!(!mentions_boolean_type("AtomicBoolean"));
        assert!(!mentions_boolean_type("boolean"));
        assert!(!mentions_boolean_type("Boolean.TRUE"));
        assert!(!mentions_boolean_type("Booleans"));
    }
}
