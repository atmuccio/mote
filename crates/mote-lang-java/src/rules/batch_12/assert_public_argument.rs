//! `java/assert-public-argument` — `assert` used to check the arguments of a public method.

use super::util::any_descendant;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Reliability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["assertions", "api"],
    remediation_minutes: 5,
    description: r#"Detects an `assert` at the start of a `public` method of a `public` class that
checks the method's parameters (`assert count >= 0;`, `assert name != null : "name";`).

**Why it matters.** Assertions are disabled unless the JVM runs with `-ea`, which production
deployments rarely do. The check therefore exists only in tests: in production a caller
passing an invalid argument is not stopped at the boundary, and the bad value travels on
until it fails somewhere unrelated, or corrupts state silently. Arguments of a public API are
exactly the values the class does not control.

**How to fix.** Validate public arguments with a check that always runs:
`Objects.requireNonNull(name, "name")`, `if (count < 0) throw new IllegalArgumentException(…)`.
Keep `assert` for internal invariants of private code.

```java
// Noncompliant
public void resize(int capacity) {
    assert capacity > 0 : "capacity must be positive";
    …
}

// Compliant
public void resize(int capacity) {
    if (capacity <= 0) {
        throw new IllegalArgumentException("capacity must be positive: " + capacity);
    }
    …
}
```

Only the leading statements of the method body are considered, and only assertions whose
condition mentions a parameter and nothing but parameters, literals and `null`. Test code is
not checked."#,
    ..RuleMeta::new(
        "java/assert-public-argument",
        "java",
        "assert used to check a public method's argument",
    )
};

pub struct AssertPublicArgument;

impl<'t> JavaRule<'t> for AssertPublicArgument {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["assert_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if ctx.is_test() {
            return;
        }
        let Some(body) = ctx.parent(node) else { return };
        let Some(method) = ctx.parent(body) else {
            return;
        };
        if !matches!(kind(body), "block" | "constructor_body")
            || !matches!(
                kind(method),
                "method_declaration" | "constructor_declaration"
            )
            || !syntax::has_modifier(method, "public")
        {
            return;
        }
        let Some(class) = ctx.parent(method).and_then(|b| ctx.parent(b)) else {
            return;
        };
        if kind(class) != "class_declaration" || !syntax::has_modifier(class, "public") {
            return;
        }
        // Leading statements only: every statement before this one is an assert too.
        for s in named_children(body) {
            if s.id() == node.id() {
                break;
            }
            if kind(s) != "assert_statement" {
                return;
            }
        }
        let params: Vec<&str> = syntax::parameters(method)
            .filter_map(|p| match kind(p) {
                "formal_parameter" => p.field(Field::Name),
                _ => None,
            })
            .map(|n| ctx.text(n))
            .collect();
        if params.is_empty() {
            return;
        }
        let Some(cond) = syntax::first_named_child(node) else {
            return;
        };
        let mut mentions_param = false;
        let mut other = false;
        let check = |n: Node<'t>, mentions_param: &mut bool, other: &mut bool| match kind(n) {
            "identifier" => {
                if params.contains(&ctx.text(n)) {
                    *mentions_param = true;
                } else {
                    *other = true;
                }
            }
            "method_invocation"
            | "field_access"
            | "this"
            | "lambda_expression"
            | "object_creation_expression"
            | "array_access"
            | "instanceof_expression" => *other = true,
            _ => {}
        };
        check(cond, &mut mentions_param, &mut other);
        any_descendant(cond, |n| {
            check(n, &mut mentions_param, &mut other);
            other
        });
        if !mentions_param || other {
            return;
        }
        let at = syntax::children(node).next().unwrap_or(node);
        ctx.emit(
            at,
            "Validate this public argument with a check that always runs (an exception); assertions are off in production",
        );
    }
}
