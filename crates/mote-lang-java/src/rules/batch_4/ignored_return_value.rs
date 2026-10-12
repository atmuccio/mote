//! `java/ignored-return-value` — result of a side-effect-free, value-returning call discarded.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["api-misuse", "immutability"],
    remediation_minutes: 2,
    description: r#"Detects a call whose only effect is to return a new value, used as a statement
so that the value is thrown away:

* `String`: `trim`, `strip*`, `toUpperCase`, `toLowerCase`, `replace*`, `substring`,
  `concat`, `repeat`, `formatted`, `indent`, and the static `String.format` / `join` /
  `valueOf`;
* `BigDecimal` / `BigInteger` arithmetic (`add`, `subtract`, `multiply`, `divide`, `pow`,
  `negate`, `abs`, `setScale`, `max`, `min`, shifts, bit operations, …);
* `java.time` values: `plus*`, `minus*`, `with*`, `truncatedTo`, `negated`,
  `multipliedBy`, …;
* `Optional`: `filter`, `or`, `orElse`, and `map` / `flatMap` with a side-effect-free lambda;
* `Stream` / `IntStream` / `LongStream` / `DoubleStream` intermediate operations (`map`,
  `filter`, `sorted`, `peek`, `limit`, …).

**Why it matters.** These types are immutable (or, for streams, lazy): the call computes a
result and changes nothing else. `name.trim();` leaves `name` untouched, `total.add(tax);`
does not update `total`, and `stream.filter(..)` without a terminal operation runs nothing.
The code reads like an update that never happens.

**How to fix.** Use the returned value — usually by assigning it back.

```java
// Noncompliant
amount.setScale(2, RoundingMode.HALF_UP);
date.plusDays(1);

// Compliant
amount = amount.setScale(2, RoundingMode.HALF_UP);
date = date.plusDays(1);
```

Only receivers whose type the resolver proves are reported. `Optional.orElseThrow`,
`orElseGet`, and `String.intern` are deliberately not included, and calls that may be there to
provoke an exception are skipped: statements in a `try` body that has a `catch`, and, in test
code, statements inside lambdas (`assertThrows(.., () -> { s.substring(9); })`). The arm of a
`switch` expression whose value is used (`return switch (mode) { case UPPER ->
s.toUpperCase(); … };`) yields the call's result instead of discarding it and is not reported
either."#,
    ..RuleMeta::new(
        "java/ignored-return-value",
        "java",
        "Result of a side-effect-free call ignored",
    )
};

/// Families of receiver types, each with its own method list.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Family {
    String,
    Big,
    Time,
    Optional,
    Stream,
}

const STRING_METHODS: &[&str] = &[
    "trim",
    "strip",
    "stripLeading",
    "stripTrailing",
    "stripIndent",
    "toUpperCase",
    "toLowerCase",
    "replace",
    "replaceAll",
    "replaceFirst",
    "substring",
    "concat",
    "repeat",
    "formatted",
    "indent",
    "translateEscapes",
];

const STRING_STATIC_METHODS: &[&str] = &["format", "join", "valueOf"];

const BIG_METHODS: &[&str] = &[
    "add",
    "subtract",
    "multiply",
    "divide",
    "divideToIntegralValue",
    "remainder",
    "pow",
    "abs",
    "negate",
    "plus",
    "round",
    "setScale",
    "max",
    "min",
    "stripTrailingZeros",
    "movePointLeft",
    "movePointRight",
    "scaleByPowerOfTen",
    "mod",
    "modPow",
    "modInverse",
    "gcd",
    "shiftLeft",
    "shiftRight",
    "and",
    "or",
    "xor",
    "not",
    "andNot",
    "setBit",
    "clearBit",
    "flipBit",
    "sqrt",
];

const TIME_EXTRA_METHODS: &[&str] = &[
    "truncatedTo",
    "negated",
    "abs",
    "multipliedBy",
    "dividedBy",
    "normalized",
];

const TIME_TYPES: &[&str] = &[
    "java.time.LocalDate",
    "java.time.LocalTime",
    "java.time.LocalDateTime",
    "java.time.ZonedDateTime",
    "java.time.OffsetDateTime",
    "java.time.OffsetTime",
    "java.time.Instant",
    "java.time.Duration",
    "java.time.Period",
    "java.time.Year",
    "java.time.YearMonth",
    "java.time.MonthDay",
];

const OPTIONAL_METHODS: &[&str] = &["filter", "or", "orElse", "map", "flatMap"];

const STREAM_METHODS: &[&str] = &[
    "filter",
    "map",
    "mapToInt",
    "mapToLong",
    "mapToDouble",
    "mapToObj",
    "flatMap",
    "distinct",
    "sorted",
    "peek",
    "limit",
    "skip",
    "takeWhile",
    "dropWhile",
    "boxed",
    "asLongStream",
    "asDoubleStream",
];

/// Every method name of the lists above (time methods are matched by prefix instead).
static CANDIDATE_NAMES: std::sync::LazyLock<FxHashSet<&'static str>> =
    std::sync::LazyLock::new(|| {
        STRING_METHODS
            .iter()
            .chain(STRING_STATIC_METHODS)
            .chain(BIG_METHODS)
            .chain(OPTIONAL_METHODS)
            .chain(STREAM_METHODS)
            .copied()
            .collect()
    });

const STREAM_TYPES: &[&str] = &[
    "java.util.stream.Stream",
    "java.util.stream.IntStream",
    "java.util.stream.LongStream",
    "java.util.stream.DoubleStream",
];

fn is_time_method(name: &str) -> bool {
    let prefixed = |p: &str| {
        name.strip_prefix(p).is_some_and(|rest| {
            rest.is_empty() || rest.starts_with(|c: char| c.is_ascii_uppercase())
        })
    };
    prefixed("plus") || prefixed("minus") || prefixed("with") || TIME_EXTRA_METHODS.contains(&name)
}

/// Per-file gates: most files never mention the less common families.
#[derive(Default)]
pub struct IgnoredReturnValue {
    big: util::Gate,
    time: util::Gate,
    optional: util::Gate,
    stream: util::Gate,
}

/// Whether an `Optional.map/flatMap/filter/or` argument is a lambda with an expression body
/// that cannot have side effects (so dropping the result loses the whole computation).
fn pure_function_arg(call: Node<'_>) -> bool {
    let mut args = util::args(call);
    let (Some(arg), None) = (args.next(), args.next()) else {
        return false;
    };
    if kind(arg) != "lambda_expression" {
        return false;
    }
    util::fld(arg, F::Body).is_some_and(|b| kind(b) != "block" && !util::has_side_effects(b))
}

impl<'t> JavaRule<'t> for IgnoredReturnValue {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["expression_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(call) = util::first_child(node) else {
            return;
        };
        if kind(call) != "method_invocation" {
            return;
        }
        let Some(name_node) = util::fld(call, F::Name) else {
            return;
        };
        let name = ctx.text(name_node);
        // One hash lookup rejects almost every statement.
        let candidate = CANDIDATE_NAMES.contains(name)
            || (self.time.open(ctx, "java.time") && is_time_method(name));
        if !candidate {
            return;
        }
        let Some(recv) = util::fld(call, F::Object) else {
            return;
        };
        if matches!(kind(recv), "this" | "super") {
            return;
        }
        // Candidate families by method name (cheap), each gated by the file text.
        let mut families: [Option<Family>; 3] = [None; 3];
        let mut n = 0;
        let mut add = |f: Family| {
            if n < families.len() {
                families[n] = Some(f);
                n += 1;
            }
        };
        if STRING_METHODS.contains(&name) || STRING_STATIC_METHODS.contains(&name) {
            add(Family::String);
        }
        if BIG_METHODS.contains(&name) && self.big.open(ctx, "java.math") {
            add(Family::Big);
        }
        if is_time_method(name) && self.time.open(ctx, "java.time") {
            add(Family::Time);
        }
        if OPTIONAL_METHODS.contains(&name)
            && self
                .optional
                .open_any(ctx, &["java.util.Optional", "java.util.*"])
        {
            add(Family::Optional);
        }
        if STREAM_METHODS.contains(&name)
            && self.stream.open_any(ctx, &["java.util.stream", "stream()"])
        {
            add(Family::Stream);
        }
        if families.iter().all(Option::is_none) {
            return;
        }
        // Static `String.format(..)` & co.
        if families.contains(&Some(Family::String))
            && STRING_STATIC_METHODS.contains(&name)
            && util::names_type(ctx, recv, "java.lang.String")
        {
            if maybe_called_for_exception(ctx, node) || yields_switch_value(ctx, node) {
                return;
            }
            ctx.emit(
                name_node,
                format!(
                    "Use the String returned by `String.{name}(..)`; the call has no other effect"
                ),
            );
            return;
        }
        let Some(t) = ctx.type_of(recv) else {
            return;
        };
        let Some(fqn) = t.fqn() else {
            return;
        };
        let family = families.iter().flatten().copied().find(|f| match f {
            Family::String => fqn == "java.lang.String" && STRING_METHODS.contains(&name),
            Family::Big => matches!(fqn, "java.math.BigDecimal" | "java.math.BigInteger"),
            Family::Time => TIME_TYPES.contains(&fqn),
            Family::Optional => {
                fqn == "java.util.Optional" && (name == "orElse" || pure_function_arg(call))
            }
            Family::Stream => STREAM_TYPES.contains(&fqn),
        });
        let Some(family) = family else {
            return;
        };
        if maybe_called_for_exception(ctx, node) || yields_switch_value(ctx, node) {
            return;
        }
        let simple = t.simple_name().unwrap_or(fqn);
        let why = match family {
            Family::Stream => "streams are lazy, so without a terminal operation nothing runs",
            _ => "the receiver is immutable, so the call has no other effect",
        };
        ctx.emit(
            name_node,
            format!("Use the value returned by `{simple}.{name}(..)`; {why}"),
        );
    }
}

/// Whether the statement may be there only to provoke an exception: directly inside the body
/// of a `try` that has a `catch` (`try { amount.setScale(2); } catch (ArithmeticException e)`),
/// or, in test code, inside a lambda (`assertThrows(.., () -> { s.substring(9); })`).
fn maybe_called_for_exception<'t>(ctx: &Ctx<'t>, stmt: Node<'t>) -> bool {
    let mut prev = stmt;
    for a in ctx.ancestors(stmt) {
        match kind(a) {
            "try_statement" | "try_with_resources_statement" => {
                let in_body = util::fld(a, F::Body).is_some_and(|b| b.id() == prev.id());
                if in_body && named_children(a).any(|c| kind(c) == "catch_clause") {
                    return true;
                }
            }
            "lambda_expression" => return ctx.is_test(),
            "method_declaration"
            | "constructor_declaration"
            | "compact_constructor_declaration"
            | "static_initializer"
            | "class_body" => return false,
            _ => {}
        }
        prev = a;
    }
    false
}

/// Whether the statement is the arm of a `switch` expression whose value is used
/// (`return switch (m) { case A -> s.trim(); … };`): tree-sitter writes the arm's expression as
/// an expression statement, but it is the value the switch yields. A `switch` in statement
/// position (directly in a block, a branch or a loop body) discards it; a switch that is itself
/// the arm of an enclosing switch is decided by that switch.
fn yields_switch_value<'t>(ctx: &Ctx<'t>, stmt: Node<'t>) -> bool {
    let mut ancestors = ctx.ancestors(stmt);
    for _ in 0..16 {
        if ancestors
            .next()
            .is_none_or(|rule| kind(rule) != "switch_rule")
        {
            return false;
        }
        let block = ancestors.next();
        let switch = ancestors.next();
        if block.is_none_or(|b| kind(b) != "switch_block")
            || switch.is_none_or(|s| kind(s) != "switch_expression")
        {
            return false;
        }
        let Some(outer) = ancestors.next() else {
            return false;
        };
        match kind(outer) {
            // A nested switch written as the arm of another switch: that switch decides.
            "expression_statement" => continue,
            "block"
            | "constructor_body"
            | "switch_block_statement_group"
            | "labeled_statement"
            | "if_statement"
            | "while_statement"
            | "do_statement"
            | "for_statement"
            | "enhanced_for_statement"
            | "program"
            | "ERROR" => return false,
            _ => return true,
        }
    }
    false
}
