//! `java/int-multiplication-overflow` — `long ms = seconds * 1000;`: an `int` product that
//! can overflow before it is widened to `long` or a floating-point type.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["numbers", "overflow"],
    params: &[ParamSpec {
        name: "min_factor",
        default: ParamDefault::Int(1000),
        description: "Smallest product of the constant factors of the multiplication for it to be reported (1 reports every int product that is widened).",
    }],
    remediation_minutes: 2,
    description: r#"Detects a multiplication of `int` values by a large constant factor whose result is
then used as a `long`, `float` or `double`: stored in such a variable, returned from such a
method, cast to it, combined with a `long` (`System.currentTimeMillis() + timeout * 1000`) or
passed to a JDK method taking a `long` time or duration (`Thread.sleep`, `TimeUnit`,
`Duration.of…`, `Instant.ofEpoch…`, `new Date(…)`).

**Why it matters.** Java evaluates `seconds * 1000` in 32 bits because both operands are
`int`; the widening to `long` happens only afterwards, when the product has already wrapped
around. A timeout of 25 days in seconds becomes a negative number of milliseconds, a size of
2048 MB in bytes becomes zero. Declaring the target as `long` shows a large value was
expected, and the compiler says nothing.

**How to fix.** Make one factor a `long` before multiplying, typically the constant
(`seconds * 1000L`), or use a conversion API (`TimeUnit.SECONDS.toMillis(seconds)`,
`Math.multiplyExact`).

```java
// Noncompliant
long timeoutMillis = timeoutSeconds * 1000;
Thread.sleep(minutes * 60 * 1000);
long bytes = sizeMb * 1024 * 1024;

// Compliant
long timeoutMillis = timeoutSeconds * 1000L;
Thread.sleep(TimeUnit.MINUTES.toMillis(minutes));
long bytes = sizeMb * 1024L * 1024;
```

Only products whose constant factors multiply to at least `min_factor` (default 1000: unit
conversions of time and sizes) are reported, and only when the resolver proves every factor
is an `int` (or narrower). Products whose factors are all constants — literals, and `final`
fields or locals of this file initialized with a constant expression (`final int count = 10;`)
— have a fixed value and are not reported (literal-only products are left to
`int-constant-overflow`); additions and products of variables alone are not reported."#,
    ..RuleMeta::new(
        "java/int-multiplication-overflow",
        "java",
        "int multiplication overflows before widening",
    )
};

pub struct IntMultiplicationOverflow;

impl<'t> JavaRule<'t> for IntMultiplicationOverflow {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["binary_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if ctx.operator_str(node) != "*" {
            return;
        }
        // The outermost `*` of a product chain reports for the whole chain.
        let (parent, child) = parent_skipping_parens(ctx, node);
        let Some(parent) = parent else { return };
        if kind(parent) == "binary_expression" && syntax::operator_str(parent) == "*" {
            return;
        }
        let mut constant: i128 = 1;
        let mut variables: Vec<Node<'t>> = Vec::new();
        if !factors(node, ctx, &mut constant, &mut variables, 0) || variables.is_empty() {
            return;
        }
        let min = i128::from(ctx.param_i64("min_factor").max(1));
        // Only a named constant can still raise the constant part.
        if constant < min && !variables.iter().any(|v| kind(*v) == "identifier") {
            return;
        }
        if !widening_context(parent, child, ctx) {
            return;
        }
        // Fold same-file constants: `static final` fields (`seconds * MILLIS_PER_SECOND`), and
        // `final` fields and locals with a constant initializer (`final int count = 10;`).
        variables.retain(|v| match const_value(*v, ctx, 0) {
            Some(c) => {
                constant = (constant * c).clamp(-i128::from(i64::MAX), i128::from(i64::MAX));
                false
            }
            None => true,
        });
        // Constant expressions are `int-constant-overflow`'s business.
        if variables.is_empty() || constant.abs() < min {
            return;
        }
        if ctx.type_of(node).and_then(|t| t.prim()) != Some(Prim::Int) {
            return;
        }
        // A `byte`/`short`/`char` times a constant may still fit: one factor must be an `int`.
        let is_int = |t: JType| t.prim().or_else(|| t.unboxed()) == Some(Prim::Int);
        if !variables
            .iter()
            .any(|v| ctx.type_of(*v).is_some_and(is_int))
        {
            return;
        }
        let at = syntax::operator(node).unwrap_or(node);
        ctx.emit(
            at,
            "Make a factor long (for example `1000L`); this int product can overflow before it is widened",
        );
    }
}

/// The enclosing node other than parentheses, and the child of it on the way to `node`.
fn parent_skipping_parens<'t>(ctx: &Ctx<'t>, node: Node<'t>) -> (Option<Node<'t>>, Node<'t>) {
    let mut child = node;
    for a in ctx.ancestors(node) {
        if kind(a) == "parenthesized_expression" {
            child = a;
            continue;
        }
        return (Some(a), child);
    }
    (None, child)
}

/// Flatten a `*` chain: multiply the decimal `int` literals into `constant` and count the
/// other factors. `false` when a factor is a `long`/floating literal or the chain is odd.
fn factors<'t>(
    node: Node<'t>,
    ctx: &Ctx<'t>,
    constant: &mut i128,
    variables: &mut Vec<Node<'t>>,
    depth: u32,
) -> bool {
    if depth > 32 {
        return false;
    }
    let node = syntax::strip_parens(node);
    match kind(node) {
        "binary_expression" if syntax::operator_str(node) == "*" => {
            let (Some(l), Some(r)) = (node.field(Field::Left), node.field(Field::Right)) else {
                return false;
            };
            factors(l, ctx, constant, variables, depth + 1)
                && factors(r, ctx, constant, variables, depth + 1)
        }
        "decimal_integer_literal" => {
            let text = ctx.text(node);
            if text.ends_with(['l', 'L']) {
                return false;
            }
            let Some(v) = syntax::int_literal_value(node, ctx.source()) else {
                return false;
            };
            *constant = (*constant * i128::from(v)).min(i128::from(i64::MAX));
            true
        }
        "hex_integer_literal"
        | "octal_integer_literal"
        | "binary_integer_literal"
        | "decimal_floating_point_literal"
        | "hex_floating_point_literal" => false,
        _ => {
            variables.push(node);
            true
        }
    }
}

fn is_wide(t: &JType) -> bool {
    matches!(
        t.prim().or_else(|| t.unboxed()),
        Some(Prim::Long | Prim::Float | Prim::Double)
    )
}

/// JDK methods whose parameter is a `long` amount of time.
const LONG_TIME_CALLS: &[(&str, &[&str])] = &[
    ("java.lang.Thread", &["sleep"]),
    (
        "java.util.concurrent.TimeUnit",
        &[
            "sleep",
            "timedJoin",
            "timedWait",
            "toNanos",
            "toMicros",
            "toMillis",
            "toSeconds",
            "toMinutes",
            "toHours",
            "toDays",
            "convert",
        ],
    ),
    (
        "java.time.Duration",
        &[
            "ofDays",
            "ofHours",
            "ofMinutes",
            "ofSeconds",
            "ofMillis",
            "ofNanos",
            "plusDays",
            "plusHours",
            "plusMinutes",
            "plusSeconds",
            "plusMillis",
            "plusNanos",
        ],
    ),
    (
        "java.time.Instant",
        &["ofEpochMilli", "ofEpochSecond", "plusMillis", "plusSeconds"],
    ),
];

/// Whether the value of `child` (a child of `parent`) is widened to `long` or a floating
/// type right away.
fn widening_context<'t>(parent: Node<'t>, child: Node<'t>, ctx: &Ctx<'t>) -> bool {
    let is = |f: Field| parent.field(f).is_some_and(|c| c.id() == child.id());
    match kind(parent) {
        "variable_declarator" if is(Field::Value) => {
            if parent.field(Field::Dimensions).is_some() {
                return false;
            }
            let Some(decl) = ctx.parent(parent) else {
                return false;
            };
            matches!(
                kind(decl),
                "local_variable_declaration" | "field_declaration" | "constant_declaration"
            ) && decl
                .field(Field::Type)
                .and_then(|t| ctx.resolve_type(t))
                .is_some_and(|t| is_wide(&t))
        }
        "assignment_expression" if is(Field::Right) => parent
            .field(Field::Left)
            .and_then(|l| ctx.type_of(l))
            .is_some_and(|t| is_wide(&t)),
        "cast_expression" if is(Field::Value) => parent
            .field(Field::Type)
            .and_then(|t| ctx.resolve_type(t))
            .is_some_and(|t| is_wide(&t)),
        "return_statement" => {
            let Some(f) = ctx.enclosing_function(parent) else {
                return false;
            };
            kind(f) == "method_declaration"
                && f.field(Field::Type)
                    .and_then(|t| ctx.resolve_type(t))
                    .is_some_and(|t| is_wide(&t))
        }
        "binary_expression" => {
            if !matches!(
                syntax::operator_str(parent),
                "+" | "-" | "<" | ">" | "<=" | ">=" | "==" | "!="
            ) {
                return false;
            }
            let other = if is(Field::Left) {
                parent.field(Field::Right)
            } else {
                parent.field(Field::Left)
            };
            other
                .and_then(|o| ctx.type_of(o))
                .is_some_and(|t| t.prim() == Some(Prim::Long))
        }
        "argument_list" => {
            let Some(call) = ctx.parent(parent) else {
                return false;
            };
            match kind(call) {
                "method_invocation" => {
                    let Some(name) = ctx.call_name(call) else {
                        return false;
                    };
                    LONG_TIME_CALLS.iter().any(|(class, names)| {
                        names.contains(&name) && ctx.is_call_to(call, class, name)
                    })
                }
                "object_creation_expression" => {
                    syntax::argument_count(call) == 1
                        && call
                            .field(Field::Type)
                            .and_then(|t| ctx.resolve_type(t))
                            .is_some_and(|t| t.is_class("java.util.Date"))
                }
                _ => false,
            }
        }
        _ => false,
    }
}

/// The value of a constant `int` expression: decimal literals, `+ - *` and same-file
/// `static final` fields initialized with one.
fn const_value<'t>(node: Node<'t>, ctx: &Ctx<'t>, depth: u32) -> Option<i128> {
    if depth > 16 {
        return None;
    }
    let node = syntax::strip_parens(node);
    match kind(node) {
        "decimal_integer_literal" => {
            if ctx.text(node).ends_with(['l', 'L']) {
                return None;
            }
            syntax::int_literal_value(node, ctx.source()).map(i128::from)
        }
        "binary_expression" => {
            let l = const_value(node.field(Field::Left)?, ctx, depth + 1)?;
            let r = const_value(node.field(Field::Right)?, ctx, depth + 1)?;
            let v = match syntax::operator_str(node) {
                "*" => l.checked_mul(r)?,
                "+" => l.checked_add(r)?,
                "-" => l.checked_sub(r)?,
                _ => return None,
            };
            (v.abs() <= i128::from(i64::MAX)).then_some(v)
        }
        "unary_expression" => {
            let v = const_value(node.field(Field::Operand)?, ctx, depth + 1)?;
            match syntax::operator_str(node) {
                "-" => Some(-v),
                "+" => Some(v),
                _ => None,
            }
        }
        "identifier" => {
            // A `final` field or local initialized with a constant expression has that value
            // for good (an initialized final field cannot be assigned again in a constructor).
            let decl = ctx.declaration_of(node)?;
            let constant = match decl.kind {
                VarKind::Field => {
                    kind(decl.statement) == "constant_declaration"
                        || syntax::has_modifier(decl.statement, "final")
                }
                VarKind::Local => syntax::has_modifier(decl.statement, "final"),
                _ => false,
            };
            if !constant || decl.decl.field(Field::Dimensions).is_some() {
                return None;
            }
            const_value(decl.value?, ctx, depth + 1)
        }
        _ => None,
    }
}
