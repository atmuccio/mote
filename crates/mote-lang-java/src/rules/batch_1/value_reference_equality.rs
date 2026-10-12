//! `java/value-reference-equality` — boxed numbers and JDK value objects compared with `==`.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["equality", "boxing"],
    remediation_minutes: 2,
    description: r#"Detects `==` and `!=` between two expressions whose static types are both boxed
numbers or characters (`Integer`, `Long`, `Short`, `Byte`, `Character`, `Double`, `Float`), or
both the same JDK value type that defines equality by content (`BigDecimal`, `BigInteger`,
`LocalDate`, `LocalTime`, `LocalDateTime`, `ZonedDateTime`, `OffsetDateTime`, `Instant`,
`Duration`, `Period`, `UUID`, `Optional`, `OptionalInt`, `OptionalLong`, `OptionalDouble`).

**Why it matters.** On objects, `==` compares references. Boxed values are only shared for a
small cache range (`Integer` from -128 to 127, for example), so `a == b` holds in tests with
small numbers and fails in production with larger ones. Value objects such as `BigDecimal` or
`LocalDate` are created fresh by nearly every operation, so two equal values are almost never
the same object.

**How to fix.** Compare with `equals()` (or `Objects.equals(a, b)` when either side may be
`null`), unbox explicitly (`a.intValue() == b.intValue()`), or, for `BigDecimal`, use
`compareTo(..) == 0` when `2.0` and `2.00` should be equal.

```java
// Noncompliant
Integer expected = order.quantity();
if (expected == stock.available()) { ... }   // both Integer

// Compliant
if (expected.equals(stock.available())) { ... }
```

Not reported: comparisons with `null` or with `Optional.empty()` (a shared instance), with
a primitive (Java unboxes and compares values), between different types (one side `Object`,
say — an identity check on purpose), `Boolean` (only two shared instances exist in practice),
enums and `Class` objects (identity is equality for them), `String` (see
`string-reference-equality`), and any operand whose type is unknown."#,
    ..RuleMeta::new(
        "java/value-reference-equality",
        "java",
        "Boxed or value objects compared with == or !=",
    )
};

pub struct ValueReferenceEquality;

/// JDK types whose instances are compared by value, and the name shown in the message.
const VALUE_TYPES: &[(&str, &str)] = &[
    ("java.lang.Integer", "Integer"),
    ("java.lang.Long", "Long"),
    ("java.lang.Short", "Short"),
    ("java.lang.Byte", "Byte"),
    ("java.lang.Character", "Character"),
    ("java.lang.Double", "Double"),
    ("java.lang.Float", "Float"),
    ("java.math.BigDecimal", "BigDecimal"),
    ("java.math.BigInteger", "BigInteger"),
    ("java.time.LocalDate", "LocalDate"),
    ("java.time.LocalTime", "LocalTime"),
    ("java.time.LocalDateTime", "LocalDateTime"),
    ("java.time.ZonedDateTime", "ZonedDateTime"),
    ("java.time.OffsetDateTime", "OffsetDateTime"),
    ("java.time.Instant", "Instant"),
    ("java.time.Duration", "Duration"),
    ("java.time.Period", "Period"),
    ("java.util.UUID", "UUID"),
    ("java.util.Optional", "Optional"),
    ("java.util.OptionalInt", "OptionalInt"),
    ("java.util.OptionalLong", "OptionalLong"),
    ("java.util.OptionalDouble", "OptionalDouble"),
];

/// Boxed numeric types: any two of them may appear on both sides (`Integer == Long` does not
/// compile, but the rule does not need to care).
fn is_boxed_numeric(fqn: &str) -> bool {
    matches!(
        fqn,
        "java.lang.Integer"
            | "java.lang.Long"
            | "java.lang.Short"
            | "java.lang.Byte"
            | "java.lang.Character"
            | "java.lang.Double"
            | "java.lang.Float"
    )
}

impl<'t> JavaRule<'t> for ValueReferenceEquality {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["binary_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let op = ctx.operator_str(node);
        if op != "==" && op != "!=" {
            return;
        }
        let (Some(left), Some(right)) = (node.field(Field::Left), node.field(Field::Right)) else {
            return;
        };
        // Cheap syntactic exclusions before any typing: `null`, literals (primitives or
        // strings), and `this`.
        let trivially_not_value = |n: Node<'t>| {
            let n = syntax::strip_parens(n);
            syntax::is_null_literal(n)
                || matches!(
                    kind(n),
                    "decimal_integer_literal"
                        | "hex_integer_literal"
                        | "octal_integer_literal"
                        | "binary_integer_literal"
                        | "decimal_floating_point_literal"
                        | "character_literal"
                        | "string_literal"
                        | "true"
                        | "false"
                        | "this"
                        | "class_literal"
                )
        };
        if trivially_not_value(left) || trivially_not_value(right) {
            return;
        }
        // `opt == Optional.empty()`: the empty optional is a shared instance.
        let is_empty_call = |n: Node<'t>| {
            let n = syntax::strip_parens(n);
            kind(n) == "method_invocation"
                && n.field(Field::Name).is_some_and(|m| ctx.text(m) == "empty")
        };
        if is_empty_call(left) || is_empty_call(right) {
            return;
        }
        let Some(lt) = ctx.type_of(left) else { return };
        let Some(lf) = lt.fqn() else { return };
        let Some((_, shown)) = VALUE_TYPES.iter().find(|(f, _)| *f == lf) else {
            return;
        };
        let Some(rt) = ctx.type_of(right) else { return };
        let Some(rf) = rt.fqn() else { return };
        let same_family = rf == lf || (is_boxed_numeric(lf) && is_boxed_numeric(rf));
        if !same_family {
            return;
        }
        let at = syntax::operator(node).unwrap_or(node);
        let msg = if op == "==" {
            format!("Compare these `{shown}` values with equals() instead of ==")
        } else {
            format!("Compare these `{shown}` values with !equals() instead of !=")
        };
        ctx.emit(at, msg);
    }
}
