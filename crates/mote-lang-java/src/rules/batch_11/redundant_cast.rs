//! `java/redundant-cast` — a cast to the type the expression already has.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["clutter", "types"],
    remediation_minutes: 1,
    description: r#"Detects a cast to exactly the static type its operand already has: a local
variable, field or parameter cast to its own declared type, a JDK method result cast to the
type the method returns, `(int)` applied to an `int` expression, `(String)` applied to a
string concatenation.

**Why it matters.** Such a cast converts nothing and can never fail, yet it reads as if a
conversion or a runtime check were taking place. It is usually left over from an earlier
version of the code, where the operand had another type (a raw collection, an `Object`
field), and it hides that history: the next reader wonders what it protects against. A
redundant numeric cast also suggests a narrowing that isn't there.

**How to fix.** Remove the cast.

```java
// Noncompliant
List<String> names = catalog.names();
String first = (String) names.get(0);
long total = (long) count * size;   // count is already a long

// Compliant
String first = names.get(0);
long total = count * size;
```

Only casts whose type and operand type the resolver knows completely are considered, down to
the arguments of the calls and the operands of the arithmetic inside the operand (an argument
of unknown type could select another overload). Not
reported: casts that change the type in any way (widening, narrowing, boxing, unboxing, from a
raw to a parameterized type or back), casts of `null`, of lambdas and of method references
(the cast gives them their type), casts to intersection types, and the inner cast of a double
cast (`(List<String>) (List) objects`, a deliberate unchecked conversion)."#,
    ..RuleMeta::new(
        "java/redundant-cast",
        "java",
        "Cast to the expression's own type",
    )
};

pub struct RedundantCast;

/// Whether every sub-expression the type of `e` depends on has a type the resolver knows: the
/// arguments and receiver of calls (an argument of unknown type could select another overload,
/// `Math.round(float)` instead of `Math.round(double)`), the operands of arithmetic, the
/// branches of a conditional.
fn well_typed<'t>(ctx: &Ctx<'t>, e: Node<'t>, depth: u32) -> bool {
    let e = syntax::strip_parens(e);
    if depth > 6 {
        return false;
    }
    let typed = |n: Node<'t>| ctx.type_of(n).is_some() && well_typed(ctx, n, depth + 1);
    match kind(e) {
        "method_invocation" => {
            if let Some(obj) = e.field(Field::Object) {
                let obj = syntax::strip_parens(obj);
                let is_type = matches!(kind(obj), "identifier" | "field_access")
                    && ctx.type_name_of(obj).is_some();
                if !is_type && !matches!(kind(obj), "this" | "super") && !typed(obj) {
                    return false;
                }
            }
            syntax::arguments(e).all(typed)
        }
        "object_creation_expression" => syntax::arguments(e).all(typed),
        "binary_expression" => {
            e.field(Field::Left).is_some_and(typed) && e.field(Field::Right).is_some_and(typed)
        }
        "unary_expression" => e.field(Field::Operand).is_some_and(typed),
        "ternary_expression" => {
            e.field(Field::Consequence).is_some_and(typed)
                && e.field(Field::Alternative).is_some_and(typed)
        }
        "array_access" => e.field(Field::Array).is_some_and(typed),
        _ => true,
    }
}

impl<'t> JavaRule<'t> for RedundantCast {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["cast_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let (Some(ty), Some(value)) = (node.field(Field::Type), node.field(Field::Value)) else {
            return;
        };
        if !matches!(
            kind(ty),
            "type_identifier"
                | "scoped_type_identifier"
                | "generic_type"
                | "integral_type"
                | "floating_point_type"
                | "boolean_type"
                | "array_type"
        ) {
            return;
        }
        let inner = syntax::strip_parens(value);
        if matches!(
            kind(inner),
            "lambda_expression" | "method_reference" | "null_literal" | "switch_expression"
        ) {
            return;
        }
        // `(List<String>) (List) objects`: the inner cast is the point of the idiom.
        if ctx
            .parent(node)
            .map(|p| {
                if kind(p) == "parenthesized_expression" {
                    ctx.parent(p).unwrap_or(p)
                } else {
                    p
                }
            })
            .is_some_and(|p| kind(p) == "cast_expression")
        {
            return;
        }
        // Cheap test for the most common operand, a plain name: one of its declarations (seen
        // so far in the file) must spell the cast type, or it is declared later (a field).
        if kind(inner) == "identifier" {
            let cast_text: String = ctx.text(ty).split_whitespace().collect();
            let name = ctx.text(inner);
            let mut declared = ctx.declared_types().types_of(name).peekable();
            if declared.peek().is_some()
                && !declared.any(|t| {
                    let t: String = ctx.text(t).split_whitespace().collect();
                    t == cast_text || util::last_segment(&t) == util::last_segment(&cast_text)
                })
            {
                return;
            }
        }
        let Some(target) = ctx.resolve_type(ty) else {
            return;
        };
        if !target.is_fully_known() || matches!(target, JType::Null | JType::Void) {
            return;
        }
        let Some(actual) = ctx.type_of(inner) else {
            return;
        };
        if !actual.is_fully_known() || actual != target || !well_typed(ctx, inner, 0) {
            return;
        }
        ctx.emit(
            ty,
            format!("Remove this cast: the expression is already of type `{target}`"),
        );
    }
}
