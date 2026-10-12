//! `java/loop-never-entered` — a loop whose condition is already false with the values the
//! loop variables were just given.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["loops", "suspicious", "dead-code"],
    remediation_minutes: 5,
    description: r#"Detects `for` and `while` loops whose condition is false on entry, judging
from the constant values the variables in it have just been given: `for (int i = 10; i < 5;
i++)`, or `int n = 0;` directly followed by `while (n > 0)`.

**Why it matters.** The body never runs. The code looks like it iterates, but it is dead: a
reversed comparison, a wrong start value, or a bound copied from another loop. Since the
variables are not compile-time constants, the compiler accepts the loop without a warning.

**How to fix.** Correct the start value or the comparison (often the loop was meant to count
down: `for (int i = 10; i > 5; i--)`), or delete the dead loop.

```java
// Noncompliant
for (int i = 10; i < 5; i++) {
    process(i);
}

// Compliant
for (int i = 10; i > 5; i--) {
    process(i);
}
```

Only integer and boolean variables initialized with literals are considered: in the `for`
initializer itself, or in the declaration immediately before a `while` loop. Conditions that
involve anything else (fields, method calls, other variables) are not evaluated."#,
    ..RuleMeta::new(
        "java/loop-never-entered",
        "java",
        "Loop condition false on entry",
    )
};

pub struct LoopNeverEntered;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Val {
    Int(i64),
    Bool(bool),
}

/// Literal value of an initializer (`5`, `-1`, `0x10`, `'a'` is not handled, `true`).
fn literal(ctx: &Ctx<'_>, n: Node<'_>) -> Option<Val> {
    let n = syntax::strip_parens(n);
    match kind(n) {
        "true" => Some(Val::Bool(true)),
        "false" => Some(Val::Bool(false)),
        _ => syntax::int_literal_value(n, ctx.source()).map(Val::Int),
    }
}

/// Collect `name = literal` from a local variable declaration of an integral or boolean type.
fn declared_values<'t>(ctx: &Ctx<'t>, decl: Node<'t>, out: &mut Vec<(&'t str, Val)>) {
    let Some(ty) = decl.field(Field::Type) else {
        return;
    };
    if !matches!(kind(ty), "integral_type" | "boolean_type") || ctx.text(ty) == "char" {
        return;
    }
    for d in syntax::declarators(decl) {
        if d.field(Field::Dimensions).is_some() {
            continue;
        }
        let (Some(name), Some(value)) = (d.field(Field::Name), d.field(Field::Value)) else {
            continue;
        };
        if let Some(v) = literal(ctx, value) {
            out.push((ctx.text(name), v));
        }
    }
}

/// Evaluate a condition with the known values; `None` when anything else is involved.
fn eval(ctx: &Ctx<'_>, e: Node<'_>, vals: &[(&str, Val)], depth: u32) -> Option<Val> {
    if depth > 16 {
        return None;
    }
    let e = syntax::strip_parens(e);
    match kind(e) {
        "identifier" => {
            let name = ctx.text(e);
            vals.iter().find(|(n, _)| *n == name).map(|(_, v)| *v)
        }
        "true"
        | "false"
        | "decimal_integer_literal"
        | "hex_integer_literal"
        | "octal_integer_literal"
        | "binary_integer_literal" => literal(ctx, e),
        "unary_expression" => {
            let operand = e.field(Field::Operand)?;
            match (
                syntax::operator_str(e),
                eval(ctx, operand, vals, depth + 1)?,
            ) {
                ("!", Val::Bool(b)) => Some(Val::Bool(!b)),
                ("-", Val::Int(i)) => i.checked_neg().map(Val::Int),
                _ => None,
            }
        }
        "binary_expression" => {
            let op = syntax::operator_str(e);
            let l = eval(ctx, e.field(Field::Left)?, vals, depth + 1);
            let r = eval(ctx, e.field(Field::Right)?, vals, depth + 1);
            // Three-valued logic: `false && ?` is false, `true || ?` is true.
            match (op, l, r) {
                ("&&", Some(Val::Bool(false)), _) | ("&&", _, Some(Val::Bool(false))) => {
                    return Some(Val::Bool(false));
                }
                ("||", Some(Val::Bool(true)), _) | ("||", _, Some(Val::Bool(true))) => {
                    return Some(Val::Bool(true));
                }
                _ => {}
            }
            let (l, r) = (l?, r?);
            match (l, r) {
                (Val::Int(a), Val::Int(b)) => Some(Val::Bool(match op {
                    "<" => a < b,
                    "<=" => a <= b,
                    ">" => a > b,
                    ">=" => a >= b,
                    "==" => a == b,
                    "!=" => a != b,
                    _ => return None,
                })),
                (Val::Bool(a), Val::Bool(b)) => Some(Val::Bool(match op {
                    "&&" | "&" => a && b,
                    "||" | "|" => a || b,
                    "==" => a == b,
                    "!=" => a != b,
                    "^" => a ^ b,
                    _ => return None,
                })),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Whether the condition mentions at least one of the variables (a condition made of
/// literals only does not compile as a loop condition when false).
fn mentions_var(ctx: &Ctx<'_>, e: Node<'_>, vals: &[(&str, Val)], depth: u32) -> bool {
    if depth > 32 {
        return false;
    }
    let e = syntax::strip_parens(e);
    if kind(e) == "identifier" {
        let t = ctx.text(e);
        return vals.iter().any(|(n, _)| *n == t);
    }
    named_children(e).any(|c| mentions_var(ctx, c, vals, depth + 1))
}

impl<'t> JavaRule<'t> for LoopNeverEntered {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["for_statement", "while_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(cond) = node.field(Field::Condition) else {
            return;
        };
        // Cheap shape test first: a comparison, a negation, or a bare name.
        let c = syntax::strip_parens(cond);
        if !matches!(
            kind(c),
            "binary_expression" | "unary_expression" | "identifier"
        ) {
            return;
        }
        let mut vals: Vec<(&'t str, Val)> = Vec::new();
        if kind(node) == "for_statement" {
            for init in named_children(node) {
                if init.start_byte() >= cond.start_byte() {
                    break;
                }
                match kind(init) {
                    "local_variable_declaration" => declared_values(ctx, init, &mut vals),
                    "assignment_expression" if syntax::operator_str(init) == "=" => {
                        let (Some(l), Some(r)) =
                            (init.field(Field::Left), init.field(Field::Right))
                        else {
                            continue;
                        };
                        if kind(l) == "identifier" {
                            if let Some(v) = literal(ctx, r) {
                                vals.push((ctx.text(l), v));
                            }
                        }
                    }
                    _ => {}
                }
            }
        } else {
            let Some(prev) = ctx.prev_named_sibling(node) else {
                return;
            };
            if kind(prev) != "local_variable_declaration" {
                return;
            }
            declared_values(ctx, prev, &mut vals);
        }
        if vals.is_empty() || !mentions_var(ctx, c, &vals, 0) {
            return;
        }
        if eval(ctx, c, &vals, 0) == Some(Val::Bool(false)) {
            ctx.emit(
                cond,
                "This loop never runs: its condition is false with the initial values",
            );
        }
    }
}
