//! `java/optional-null` — an `Optional` compared with `null` or set to `null` (and, as an
//! option, `Optional` parameters).

use super::util::Gate;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["optional", "null-safety"],
    params: &[ParamSpec {
        name: "report_parameters",
        default: ParamDefault::Bool(false),
        description: "Also report method and constructor parameters declared as `Optional` (a design choice rather than a defect).",
    }],
    remediation_minutes: 5,
    description: r#"Detects an `Optional` (`java.util.Optional`, `OptionalInt`, `OptionalLong`,
`OptionalDouble`, or Guava's `Optional`) that is compared with `null` (`opt == null`,
`opt != null`) or given the value `null` (`Optional<User> user = null;`, `user = null;`).
With `report_parameters`, method and constructor parameters declared as `Optional` are
reported as well.

**Why it matters.** An `Optional` exists to represent "no value" without `null`. A variable of
that type holding `null` breaks the contract every caller relies on: `isPresent()`, `map(..)`
or `orElse(..)` on it throw `NullPointerException` instead of handling the empty case. A
comparison with `null` is either dead code or a sign that such a `null` does circulate, and it
often stands where `isPresent()` / `isEmpty()` was meant. An `Optional` parameter forces every
caller to wrap its argument and still admits `null`, so overloads or a nullable parameter are
usually clearer.

**How to fix.** Use `Optional.empty()` instead of `null`, test with `isPresent()` /
`isEmpty()`, and make sure no method returns `null` for an `Optional`.

```java
// Noncompliant
Optional<User> user = null;
if (lookup != null && lookup.isPresent()) { … }

// Compliant
Optional<User> user = Optional.empty();
if (lookup.isPresent()) { … }
```

Only expressions whose type the resolver proves to be one of these optional types are
reported; `return null` from a method returning `Optional` is `optional-return-null`'s. Parameter
reporting skips `@Override` methods, whose signature is inherited, and annotated parameters
(`@RequestParam Optional<String> q`), which frameworks fill in."#,
    ..RuleMeta::new(
        "java/optional-null",
        "java",
        "Optional compared with or set to null",
    )
};

const OPTIONALS: &[&str] = &[
    "java.util.Optional",
    "java.util.OptionalInt",
    "java.util.OptionalLong",
    "java.util.OptionalDouble",
    "com.google.common.base.Optional",
];

fn is_optional(t: &JType) -> bool {
    t.fqn().is_some_and(|f| OPTIONALS.contains(&f))
}

#[derive(Default)]
pub struct OptionalNull {
    gate: Gate,
}

impl<'t> JavaRule<'t> for OptionalNull {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "binary_expression",
            "variable_declarator",
            "assignment_expression",
            "formal_parameter",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !self.gate.open(ctx, "Optional") {
            return;
        }
        match kind(node) {
            "binary_expression" => {
                if !matches!(ctx.operator_str(node), "==" | "!=") {
                    return;
                }
                let (Some(l), Some(r)) = (node.field(Field::Left), node.field(Field::Right)) else {
                    return;
                };
                let other = if syntax::is_null_literal(r) {
                    l
                } else if syntax::is_null_literal(l) {
                    r
                } else {
                    return;
                };
                if ctx.type_of(other).is_some_and(|t| is_optional(&t)) {
                    ctx.emit(
                        node,
                        "Test this Optional with isPresent() or isEmpty(); an Optional should never be null",
                    );
                }
            }
            "variable_declarator" => {
                let Some(value) = node.field(Field::Value) else {
                    return;
                };
                if !syntax::is_null_literal(syntax::strip_parens(value)) {
                    return;
                }
                let Some(decl) = ctx.parent(node) else { return };
                if !matches!(
                    kind(decl),
                    "local_variable_declaration" | "field_declaration"
                ) {
                    return;
                }
                if decl
                    .field(Field::Type)
                    .and_then(|t| ctx.resolve_type(t))
                    .is_some_and(|t| is_optional(&t))
                {
                    ctx.emit(
                        value,
                        "Initialize this Optional with Optional.empty() instead of null",
                    );
                }
            }
            "assignment_expression" => {
                if syntax::operator_str(node) != "=" {
                    return;
                }
                let (Some(l), Some(r)) = (node.field(Field::Left), node.field(Field::Right)) else {
                    return;
                };
                if !syntax::is_null_literal(syntax::strip_parens(r)) {
                    return;
                }
                if ctx.type_of(l).is_some_and(|t| is_optional(&t)) {
                    ctx.emit(
                        r,
                        "Assign Optional.empty() instead of null to this Optional",
                    );
                }
            }
            _ => {
                if !ctx.param_bool("report_parameters") {
                    return;
                }
                let Some(ty) = node.field(Field::Type) else {
                    return;
                };
                if !ctx.text(ty).contains("Optional") {
                    return;
                }
                // `@RequestParam Optional<String> q`: framework-bound parameters are idiomatic.
                if syntax::annotations(node).next().is_some() {
                    return;
                }
                let Some(params) = ctx.parent(node) else {
                    return;
                };
                let Some(method) = ctx.parent(params) else {
                    return;
                };
                if !matches!(
                    kind(method),
                    "method_declaration" | "constructor_declaration"
                ) || syntax::has_annotation(method, ctx.source(), "Override")
                {
                    return;
                }
                if ctx.resolve_type(ty).is_some_and(|t| is_optional(&t)) {
                    ctx.emit(
                        ty,
                        "Take the value (or null) instead of an Optional parameter; callers must wrap arguments and can still pass null",
                    );
                }
            }
        }
    }
}
