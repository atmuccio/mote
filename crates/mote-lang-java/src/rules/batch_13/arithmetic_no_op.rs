//! `java/arithmetic-no-op` — arithmetic that leaves its operand unchanged: `x + 0`, `x * 1`,
//! `x & -1`, rounding a whole number.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["arithmetic", "redundant", "suspicious"],
    remediation_minutes: 2,
    description: r#"Detects operations that cannot change the value of an `int` or `long` operand:
adding, subtracting, or-ing or xor-ing `0`, multiplying or dividing by `1`, and-ing with `-1`
(`0xFFFFFFFF`), the same as compound assignments (`x += 0`, `x *= 1`), multiplying or dividing
a `float` or `double` by `1`, and `Math.round`, `ceil`, `floor` or `rint` of an integer value.

**Why it matters.** Such an operation is at best noise that makes the reader look for a
meaning that is not there, and often the trace of a mistake: a constant that was meant to be
something else, an offset that was never filled in, a mask that was meant to be `long`
(`value & 0xFFFFFFFF` keeps all 64 bits of a `long`, because the `int` literal is `-1`), or a
rounding applied after an integer division has already truncated the result
(`Math.ceil(total / pageSize)` never rounds up).

**How to fix.** Remove the operation, or write what was meant: `0xFFFFFFFFL` for a 32-bit
mask, a floating-point division before rounding (`Math.ceil((double) total / pageSize)`).

```java
// Noncompliant
int pages = (int) Math.ceil(total / pageSize);
long low = value & 0xFFFFFFFF;

// Compliant
int pages = (int) Math.ceil((double) total / pageSize);
long low = value & 0xFFFFFFFFL;
```

Not reported: `char`, `byte` and `short` operands (`c + 0` is the usual way to get a
character's code as an `int`), operations whose literal changes the type (`x + 0L`,
`x * 1.0`), adding `0.0` to a floating-point value (it turns `-0.0` into `0.0`), shifts,
expressions made only of literals (`1 * 60 * 1000`), and aligned series where the same
expression is combined with other literals nearby (`buf[off + 0] = a; buf[off + 1] = b;`)."#,
    ..RuleMeta::new(
        "java/arithmetic-no-op",
        "java",
        "Arithmetic operation without effect",
    )
};

pub struct ArithmeticNoOp;

/// The value of a no-op literal: `0`, `1`, `-1` (int), `-1L` (long, as `0xFFFF…FFFFL`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Lit {
    Zero,
    One,
    MinusOneInt,
    MinusOneLong,
}

fn literal(ctx: &Ctx<'_>, n: Node<'_>) -> Option<Lit> {
    let n = syntax::strip_parens(n);
    match kind(n) {
        "decimal_integer_literal" => match ctx.text(n) {
            "0" => Some(Lit::Zero),
            "1" => Some(Lit::One),
            _ => None,
        },
        "hex_integer_literal" => {
            let t = ctx.text(n).to_ascii_lowercase().replace('_', "");
            match t.as_str() {
                "0xffffffff" => Some(Lit::MinusOneInt),
                "0xffffffffffffffffl" => Some(Lit::MinusOneLong),
                _ => None,
            }
        }
        "unary_expression" => {
            let op = syntax::operator_str(n);
            let operand = n.field(Field::Operand)?;
            (op == "-" && kind(operand) == "decimal_integer_literal" && ctx.text(operand) == "1")
                .then_some(Lit::MinusOneInt)
        }
        _ => None,
    }
}

/// Whether an expression is made only of literals (a constant the compiler folds).
fn literal_only(n: Node<'_>, depth: u32) -> bool {
    if depth > 16 {
        return false;
    }
    let n = syntax::strip_parens(n);
    match kind(n) {
        "decimal_integer_literal"
        | "hex_integer_literal"
        | "octal_integer_literal"
        | "binary_integer_literal"
        | "decimal_floating_point_literal"
        | "hex_floating_point_literal"
        | "character_literal" => true,
        "unary_expression" => n
            .field(Field::Operand)
            .is_some_and(|o| literal_only(o, depth + 1)),
        "binary_expression" => {
            n.field(Field::Left)
                .is_some_and(|o| literal_only(o, depth + 1))
                && n.field(Field::Right)
                    .is_some_and(|o| literal_only(o, depth + 1))
        }
        _ => false,
    }
}

/// Which literal makes `op` a no-op, and on which sides it may stand.
fn neutral(op: &str) -> Option<(&'static [Lit], bool)> {
    // (literals, literal allowed on the left)
    Some(match op {
        "+" | "|" | "^" => (&[Lit::Zero], true),
        "-" => (&[Lit::Zero], false),
        "*" => (&[Lit::One], true),
        "/" => (&[Lit::One], false),
        "&" => (&[Lit::MinusOneInt, Lit::MinusOneLong], true),
        _ => return None,
    })
}

/// Whether the operand type makes the operation a no-op for this literal.
fn type_ok(t: Prim, op: &str, lit: Lit) -> bool {
    match t {
        Prim::Int => lit != Lit::MinusOneLong,
        Prim::Long => true,
        Prim::Float | Prim::Double => matches!(op, "*" | "/") && lit == Lit::One,
        _ => false,
    }
}

impl<'t> JavaRule<'t> for ArithmeticNoOp {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "binary_expression",
            "assignment_expression",
            "method_invocation",
        ]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["round", "ceil", "floor", "rint"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        match kind(node) {
            "binary_expression" => check_binary(node, ctx),
            "assignment_expression" => check_assignment(node, ctx),
            _ => check_rounding(node, ctx),
        }
    }
}

fn check_binary<'t>(node: Node<'t>, ctx: &Ctx<'t>) {
    let op = ctx.operator_str(node);
    let Some((lits, left_ok)) = neutral(op) else {
        return;
    };
    let (Some(l), Some(r)) = (node.field(Field::Left), node.field(Field::Right)) else {
        return;
    };
    let (lit_node, lit, other, lit_left) =
        if let Some(v) = literal(ctx, r).filter(|v| lits.contains(v)) {
            (r, v, l, false)
        } else if let Some(v) = literal(ctx, l).filter(|v| left_ok && lits.contains(v)) {
            (l, v, r, true)
        } else {
            return;
        };
    if literal_only(other, 0) {
        return;
    }
    let Some(t) = ctx.type_of(other).and_then(|t| t.prim()) else {
        return;
    };
    if !type_ok(t, op, lit) {
        return;
    }
    // `1 * 60 * x`, `x * 1 * 1024`: part of a product of constants.
    if let Some(p) = ctx.parent(node) {
        if kind(p) == "binary_expression" && syntax::operator_str(p) == op {
            let sibling = if p.field(Field::Left).map(|n| n.id()) == Some(node.id()) {
                p.field(Field::Right)
            } else {
                p.field(Field::Left)
            };
            if sibling.is_some_and(|s| literal_only(s, 0)) {
                return;
            }
        }
    }
    if !certain(ctx, other, 0) {
        return;
    }
    let lit_text = ctx.text(lit_node);
    let hex = lit_text.starts_with("0x") || lit_text.starts_with("0X");
    // A 32-bit mask on a `long` is a bug even in an aligned series.
    let long_mask = lit == Lit::MinusOneInt && t == Prim::Long && hex;
    let other_text = compact(ctx.text(other));
    if !long_mask && aligned(ctx, node, &other_text, op, lit_left, lit_text) {
        return;
    }
    let msg = if long_mask {
        format!("Write `{lit_text}L` for a 32-bit mask: `{lit_text}` is the int -1 and keeps all 64 bits")
    } else {
        format!("Remove `{op} {lit_text}`: it does not change the value")
    };
    let at = syntax::operator(node).unwrap_or(node);
    ctx.emit(at, msg);
}

fn check_assignment<'t>(node: Node<'t>, ctx: &Ctx<'t>) {
    let op = syntax::operator_str(node);
    let bin = match op {
        "+=" => "+",
        "-=" => "-",
        "|=" => "|",
        "^=" => "^",
        "*=" => "*",
        "/=" => "/",
        "&=" => "&",
        _ => return,
    };
    let Some((lits, _)) = neutral(bin) else {
        return;
    };
    let (Some(l), Some(r)) = (node.field(Field::Left), node.field(Field::Right)) else {
        return;
    };
    let Some(lit) = literal(ctx, r).filter(|v| lits.contains(v)) else {
        return;
    };
    let Some(t) = ctx.type_of(l).and_then(|t| t.prim()) else {
        return;
    };
    if !type_ok(t, bin, lit) {
        return;
    }
    let left_text = compact(ctx.text(l));
    if aligned(ctx, node, &left_text, op, false, ctx.text(r)) {
        return;
    }
    let at = syntax::operator(node).unwrap_or(node);
    ctx.emit(
        at,
        format!(
            "Remove this `{op} {}`: it does not change `{}`",
            ctx.text(r),
            ctx.text(l)
        ),
    );
}

fn check_rounding<'t>(node: Node<'t>, ctx: &Ctx<'t>) {
    let Some(name) = ctx.call_name(node) else {
        return;
    };
    if !matches!(name, "round" | "ceil" | "floor" | "rint") || syntax::argument_count(node) != 1 {
        return;
    }
    let Some(recv) = node.field(Field::Object) else {
        return;
    };
    if !matches!(
        ctx.text(recv),
        "Math" | "StrictMath" | "java.lang.Math" | "java.lang.StrictMath"
    ) {
        return;
    }
    let Some(arg) = util::arg(node, 0) else {
        return;
    };
    let Some(t) = ctx.type_of(arg).and_then(|t| t.prim()) else {
        return;
    };
    if !t.is_integral() || !certain(ctx, arg, 0) {
        return;
    }
    if !ctx.is_call_to(node, "java.lang.Math", name)
        && !ctx.is_call_to(node, "java.lang.StrictMath", name)
    {
        return;
    }
    let inner = syntax::strip_parens(arg);
    let msg = if kind(inner) == "binary_expression" && syntax::operator_str(inner) == "/" {
        format!("Divide as floating point before `{name}`: `{}` is an integer division that has already truncated", ctx.text(inner))
    } else {
        format!("Remove `{name}`: its argument is already a whole number")
    };
    let at = node.field(Field::Name).unwrap_or(node);
    ctx.emit(at, msg);
}

/// The text of an expression without whitespace.
fn compact(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

/// Whether the expression is part of an aligned series: the same operand combined with a
/// different literal by the same operator elsewhere in the enclosing method (or class, for a
/// field): `buf[off + 0] = a; buf[off + 1] = b;`, `cells[i + 0]` … `cells[i + 6]`.
fn aligned<'t>(
    ctx: &Ctx<'t>,
    node: Node<'t>,
    operand: &str,
    op: &str,
    lit_left: bool,
    lit: &str,
) -> bool {
    let scope = ctx
        .enclosing_function(node)
        .or_else(|| ctx.enclosing_class(node))
        .unwrap_or(node);
    other_literal(
        &compact(ctx.text(scope)),
        operand,
        op,
        lit_left,
        &compact(lit),
    )
}

/// Whether compact `text` contains `operand op L` (or `L op operand`) with a numeric literal
/// `L` other than `lit`.
fn other_literal(text: &str, operand: &str, op: &str, lit_left: bool, lit: &str) -> bool {
    if operand.is_empty() {
        return false;
    }
    let is_ident = |c: char| c.is_alphanumeric() || c == '_' || c == '$' || c == '.';
    let differs = |token: &str| {
        token
            .trim_start_matches('-')
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
            && !token.eq_ignore_ascii_case(lit)
    };
    let mut from = 0;
    while let Some(pos) = text[from..].find(operand) {
        let start = from + pos;
        let end = start + operand.len();
        from = start + operand.chars().next().map_or(1, char::len_utf8);
        let before = &text[..start];
        let after = &text[end..];
        if lit_left {
            if after.chars().next().is_some_and(is_ident) {
                continue;
            }
            if let Some(rest) = before.strip_suffix(op) {
                let tok_start = rest
                    .char_indices()
                    .rev()
                    .take_while(|(_, c)| c.is_alphanumeric() || *c == '_' || *c == '.')
                    .last()
                    .map_or(rest.len(), |(i, _)| i);
                if differs(&rest[tok_start..]) {
                    return true;
                }
            }
        } else {
            if before.chars().next_back().is_some_and(is_ident) {
                continue;
            }
            if let Some(rest) = after.strip_prefix(op) {
                let sign = usize::from(rest.starts_with('-'));
                let tok_end = rest[sign..]
                    .char_indices()
                    .find(|(_, c)| !(c.is_alphanumeric() || *c == '_' || *c == '.'))
                    .map_or(rest.len(), |(i, _)| i + sign);
                if differs(&rest[..tok_end]) {
                    return true;
                }
            }
        }
    }
    false
}

/// Whether the resolver's type for an expression does not depend on a guessed overload: every
/// method call in it has arguments of known types.
fn certain<'t>(ctx: &Ctx<'t>, expr: Node<'t>, depth: u32) -> bool {
    if depth > 8 {
        return false;
    }
    let e = syntax::strip_parens(expr);
    match kind(e) {
        "identifier"
        | "field_access"
        | "this"
        | "cast_expression"
        | "decimal_integer_literal"
        | "hex_integer_literal"
        | "octal_integer_literal"
        | "binary_integer_literal"
        | "decimal_floating_point_literal"
        | "character_literal" => true,
        "array_access" => e
            .field(Field::Array)
            .is_some_and(|a| certain(ctx, a, depth + 1)),
        "update_expression" => {
            syntax::first_named_child(e).is_some_and(|o| certain(ctx, o, depth + 1))
        }
        "unary_expression" => e
            .field(Field::Operand)
            .is_some_and(|o| certain(ctx, o, depth + 1)),
        "binary_expression" => {
            e.field(Field::Left)
                .is_some_and(|o| certain(ctx, o, depth + 1))
                && e.field(Field::Right)
                    .is_some_and(|o| certain(ctx, o, depth + 1))
        }
        "method_invocation" => util::args(e).all(|a| {
            certain(ctx, a, depth + 1) && ctx.type_of(a).is_some_and(|t| t.is_fully_known())
        }),
        _ => false,
    }
}
