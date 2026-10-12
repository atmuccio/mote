//! `java/conditional-mixed-boxed-numbers` — `c ? anInteger : aLong` unboxes both operands
//! and widens the result.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["boxing", "suspicious"],
    remediation_minutes: 5,
    description: r#"Detects conditional expressions (`c ? a : b`) whose two results are numbers of
different types where at least one is a wrapper object, and the code treats the result as an
object: both operands are wrappers (`Integer` and `Long`), or one is a wrapper and the
result is stored in an `Object`, `Number`, `Comparable` or `Serializable` variable.

**Why it matters.** When the operands of `?:` are different numeric types, Java applies
numeric promotion: it unboxes the wrappers and widens the value to the larger type. The
result is a primitive `long` or `double`, boxed again as a `Long` or `Double` if an object is
needed. Two surprises follow: an operand that is `null` throws a `NullPointerException`
although neither side is primitive, and the chosen `Integer` comes out as a `Long` (or `1`
becomes `1.0`), which breaks later `equals` checks, `instanceof` tests and map lookups.

**How to fix.** Give both operands the same type (convert one explicitly), or use an
`if`/`else` that assigns each object as it is.

```java
// Noncompliant
Number n = useLimit ? limit : total;     // Integer limit, Long total: limit becomes a Long

// Compliant
Number n;
if (useLimit) {
    n = limit;
} else {
    n = total;
}
```

Both operand types must be proven by the resolver, and at least one must show in its syntax
(a literal, or a variable with a declared type). Conditionals whose operands are both
primitives, or whose result goes to a primitive variable (where the conversion is expected),
are not reported."#,
    ..RuleMeta::new(
        "java/conditional-mixed-boxed-numbers",
        "java",
        "Conditional mixes different boxed number types",
    )
};

pub struct ConditionalMixedBoxedNumbers;

/// Operand kinds that can never be numbers: rejected without resolving anything.
fn never_numeric(n: Node<'_>) -> bool {
    matches!(
        kind(syntax::strip_parens(n)),
        "string_literal"
            | "null_literal"
            | "true"
            | "false"
            | "lambda_expression"
            | "method_reference"
            | "array_creation_expression"
            | "instanceof_expression"
            | "class_literal"
            | "this"
    )
}

/// The declared type text of the variable the conditional initializes or is assigned to, or
/// the return type of the method it is returned from.
fn target_type_text<'t>(ctx: &Ctx<'t>, ternary: Node<'t>) -> Option<&'t str> {
    let mut cur = ternary;
    let mut parent = ctx.parent(cur)?;
    while kind(parent) == "parenthesized_expression" {
        cur = parent;
        parent = ctx.parent(cur)?;
    }
    match kind(parent) {
        "variable_declarator" => {
            let decl = ctx.parent(parent)?;
            decl.field(Field::Type).map(|t| ctx.text(t))
        }
        "assignment_expression" if syntax::operator_str(parent) == "=" => {
            let left = parent.field(Field::Left)?;
            if kind(left) != "identifier" {
                return None;
            }
            ctx.declared_types()
                .type_of_name(ctx.text(left))
                .map(|t| ctx.text(t))
        }
        "return_statement" => {
            let m = ctx.enclosing_function(parent)?;
            if kind(m) != "method_declaration" {
                return None;
            }
            m.field(Field::Type).map(|t| ctx.text(t))
        }
        _ => None,
    }
}

impl<'t> JavaRule<'t> for ConditionalMixedBoxedNumbers {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["ternary_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let (Some(a), Some(b)) = (
            node.field(Field::Consequence),
            node.field(Field::Alternative),
        ) else {
            return;
        };
        if never_numeric(a) || never_numeric(b) {
            return;
        }
        // Cheap syntactic view of each arm before any type resolution.
        let (ka, kb) = (arm_kind(ctx, a), arm_kind(ctx, b));
        if ka == Arm::Other
            || kb == Arm::Other
            || (ka == Arm::Primitive && kb == Arm::Primitive)
            || (ka == Arm::Unknown && kb == Arm::Unknown)
        {
            return;
        }
        let Some(ta) = ctx.type_of(a) else { return };
        if !ta.is_numeric() {
            return;
        }
        let Some(tb) = ctx.type_of(b) else { return };
        if !tb.is_numeric() {
            return;
        }
        let (Some(pa), Some(pb)) = (ta.unboxed(), tb.unboxed()) else {
            return;
        };
        if pa == pb || (!ta.is_boxed() && !tb.is_boxed()) {
            return;
        }
        let both_boxed = ta.is_boxed() && tb.is_boxed();
        let target = target_type_text(ctx, node).map(|t| {
            let simple = t.rsplit('.').next().unwrap_or(t);
            simple.split('<').next().unwrap_or(simple).trim()
        });
        let object_target = target
            .is_some_and(|b| matches!(b, "Object" | "Number" | "Comparable" | "Serializable"));
        let primitive_target = target.is_some_and(|b| {
            matches!(
                b,
                "int" | "long" | "short" | "byte" | "char" | "double" | "float" | "var"
            )
        });
        if primitive_target || !(both_boxed || object_target) {
            return;
        }
        let wider = crate::resolve::Prim::promote(pa, pb);
        ctx.emit(
            node,
            format!(
                "This conditional unboxes its `{ta}` and `{tb}` operands and yields a `{}`: a null operand throws, and the other value changes type",
                wider.name()
            ),
        );
    }
}

/// What an arm of the conditional can be, judged from its syntax and declared types.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Arm {
    /// A numeric literal or a variable declared with a primitive numeric type.
    Primitive,
    /// A variable declared with a wrapper type.
    Boxed,
    /// Certainly not a number (declared with another type).
    Other,
    /// Needs the resolver.
    Unknown,
}

fn arm_kind<'t>(ctx: &Ctx<'t>, arm: Node<'t>) -> Arm {
    let arm = syntax::strip_parens(arm);
    match kind(arm) {
        "decimal_integer_literal"
        | "hex_integer_literal"
        | "octal_integer_literal"
        | "binary_integer_literal"
        | "decimal_floating_point_literal"
        | "character_literal" => Arm::Primitive,
        "identifier" => {
            let Some(t) = ctx.declared_types().type_of_name(ctx.text(arm)) else {
                return Arm::Unknown;
            };
            let text = ctx.text(t);
            let simple = text.rsplit('.').next().unwrap_or(text);
            match simple {
                "int" | "long" | "short" | "byte" | "char" | "double" | "float" => Arm::Primitive,
                "Integer" | "Long" | "Short" | "Byte" | "Character" | "Double" | "Float" => {
                    Arm::Boxed
                }
                "var" | "Number" | "Object" => Arm::Unknown,
                _ if kind(t) == "type_identifier" && simple.len() == 1 => Arm::Unknown,
                _ => Arm::Other,
            }
        }
        _ => Arm::Unknown,
    }
}
