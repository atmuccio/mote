//! `java/float-loop-counter` — `for (double x = 0; x != 1; x += 0.1)`.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["suspicious", "floating-point"],
    remediation_minutes: 5,
    description: r#"Detects `for` loops whose counter is a `float` or `double` (declared in
the loop header, tested in its condition, and updated in its update part).

**Why it matters.** Most decimal steps (`0.1`, `0.01`) have no exact binary representation, so
the counter drifts: after ten steps of `0.1` it is `0.9999999999999999`, not `1.0`. The number
of iterations then depends on rounding — the loop may run once more or once less than
intended, or never end when it tests for equality. With `float`, large counters stop
changing altogether once the step is below their precision.

**How to fix.** Count with an integer and compute the floating-point value from it.

```java
// Noncompliant
for (double x = 0.0; x <= 1.0; x += 0.1) {
    plot(x);
}

// Compliant
for (int i = 0; i <= 10; i++) {
    plot(i / 10.0);
}
```

Not reported: floating-point variables that the loop header declares but does not both test
and update (an accumulator in a loop that ends with `break`), and `for`-each loops over
collections of numbers."#,
    ..RuleMeta::new(
        "java/float-loop-counter",
        "java",
        "Floating-point loop counter",
    )
};

pub struct FloatLoopCounter;

/// Whether the declared type (or, for `var`, the initializer) is floating-point.
fn is_floating<'t>(ctx: &Ctx<'t>, ty: Node<'t>, value: Option<Node<'t>>) -> bool {
    match kind(ty) {
        "floating_point_type" => true,
        "type_identifier" => match ctx.text(ty) {
            "Float" | "Double" | "java.lang.Float" | "java.lang.Double" => true,
            "var" => value.is_some_and(|v| {
                matches!(
                    kind(syntax::strip_parens(v)),
                    "decimal_floating_point_literal" | "hex_floating_point_literal"
                )
            }),
            _ => false,
        },
        _ => false,
    }
}

/// Whether `text` contains `name` as a whole word.
fn mentions(text: &str, name: &str) -> bool {
    let is_ident = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    text.match_indices(name).any(|(i, _)| {
        !text[..i].chars().next_back().is_some_and(is_ident)
            && !text[i + name.len()..].chars().next().is_some_and(is_ident)
    })
}

/// Whether one of the loop's update expressions assigns or increments `name`.
fn updates<'t>(ctx: &Ctx<'t>, for_stmt: Node<'t>, name: &str) -> bool {
    let mut cursor = for_stmt.walk();
    let found = for_stmt
        .children_by_field_name("update", &mut cursor)
        .any(|u| {
            let target = match kind(u) {
                "assignment_expression" => u.field(Field::Left),
                "update_expression" => syntax::first_named_child(u),
                _ => None,
            };
            target.is_some_and(|t| ctx.text(syntax::strip_parens(t)) == name)
        });
    found
}

impl<'t> JavaRule<'t> for FloatLoopCounter {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["for_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(init) = node.field(Field::Init) else {
            return;
        };
        if kind(init) != "local_variable_declaration" {
            return;
        }
        let Some(ty) = init.field(Field::Type) else {
            return;
        };
        for decl in syntax::declarators(init) {
            if !is_floating(ctx, ty, decl.field(Field::Value)) {
                continue;
            }
            let Some(name) = decl.field(Field::Name) else {
                continue;
            };
            let n = ctx.text(name);
            let tested = node
                .field(Field::Condition)
                .is_some_and(|c| mentions(ctx.text(c), n));
            if tested && updates(ctx, node, n) {
                ctx.emit(
                    name,
                    format!(
                        "Count this loop with an integer: the floating-point counter `{n}` \
                         accumulates rounding errors"
                    ),
                );
                return;
            }
        }
    }
}
