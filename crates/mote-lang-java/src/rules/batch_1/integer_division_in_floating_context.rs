//! `java/integer-division-in-floating-context` — `double ratio = count / total;`.

use super::util::{self, Fld, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["numbers", "precision"],
    remediation_minutes: 5,
    description: r#"Detects an integer division (both operands `int`/`long`/`short`/`byte`/`char`)
whose result is immediately stored in, returned as, or cast to a `float` or `double`.

**Why it matters.** Integer division truncates *before* the result is widened, so
`double half = 1 / 2;` is `0.0`, and `double ratio = done / total;` is `0.0` until the job is
complete. Declaring the target as `double` suggests a fractional result was expected.

**How to fix.** Convert an operand before dividing (`(double) done / total`, `done / 2.0`). If
truncation is intended, make it explicit (`Math.floorDiv`, or store the quotient in an integer
variable first).

```java
// Noncompliant
double ratio = done / total;
double average() { return sum / count; }

// Compliant
double ratio = (double) done / total;
double average() { return (double) sum / count; }
```

Only direct uses are reported: a variable or field initializer, an assignment (including
compound ones such as `+=`), a `return` from a method declared to return `float`/`double`, and a
cast `(double) (a / b)`. Returns inside lambdas and divisions nested in larger arithmetic are
not reported."#,
    ..RuleMeta::new(
        "java/integer-division-in-floating-context",
        "java",
        "Integer division in a floating-point context",
    )
};

pub struct IntegerDivisionInFloatingContext;

fn is_integral(t: &JType) -> bool {
    t.prim()
        .or_else(|| t.unboxed())
        .is_some_and(|p| p.is_integral())
}

/// The declared type of the variable a `variable_declarator` belongs to.
fn declarator_type<'t>(ctx: &Ctx<'t>, declarator: Node<'t>) -> Option<JType> {
    if declarator.fld(F::Dimensions).is_some() {
        return None;
    }
    let decl = ctx.parent(declarator)?;
    if !matches!(
        kind(decl),
        "local_variable_declaration" | "field_declaration" | "constant_declaration"
    ) {
        return None;
    }
    ctx.resolve_type(decl.fld(F::Type)?)
}

impl<'t> JavaRule<'t> for IntegerDivisionInFloatingContext {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["binary_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if ctx.operator_str(node) != "/" {
            return;
        }
        let Some((parent, child)) = util::parent_skipping_parens(ctx, node) else {
            return;
        };
        let is_field = |f: F| parent.fld(f).is_some_and(|c| c.id() == child.id());
        let context: Option<JType> = match kind(parent) {
            "variable_declarator" if is_field(F::Value) => declarator_type(ctx, parent),
            "assignment_expression" if is_field(F::Right) => {
                parent.fld(F::Left).and_then(|l| ctx.type_of(l))
            }
            "cast_expression" if is_field(F::Value) => {
                parent.fld(F::Type).and_then(|t| ctx.resolve_type(t))
            }
            "return_statement" => {
                // Must return from a method (not a lambda) declared as float/double.
                let Some(f) = ctx.enclosing_function(parent) else {
                    return;
                };
                if kind(f) != "method_declaration" {
                    return;
                }
                f.fld(F::Type).and_then(|t| ctx.resolve_type(t))
            }
            _ => return,
        };
        let Some(target) = context else { return };
        if !util::is_floating_primitive(&target) {
            return;
        }
        let (Some(left), Some(right)) = (node.fld(F::Left), node.fld(F::Right)) else {
            return;
        };
        let Some(lt) = ctx.type_of(left) else { return };
        if !is_integral(&lt) {
            return;
        }
        let Some(rt) = ctx.type_of(right) else { return };
        if !is_integral(&rt) {
            return;
        }
        let at = util::op(node).unwrap_or(node);
        let ty = target.prim().map(|p| p.name()).unwrap_or("double");
        ctx.emit(
            at,
            format!("Cast an operand to {ty} before dividing; this integer division truncates the result"),
        );
    }
}
