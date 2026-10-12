//! `java/assertion-description-after-check` — an AssertJ description (`as(..)`,
//! `describedAs(..)`, `withFailMessage(..)`) placed after the check it was meant to describe.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["tests", "assertj"],
    remediation_minutes: 2,
    description: r#"Detects AssertJ assertions that end with a description or failure-message call
placed after the check: `assertThat(total).isEqualTo(10).as("order total")`.

**Why it matters.** AssertJ runs each check as soon as it is called. A description set
afterwards is attached to an assertion that has already passed or failed: when the check
fails, the error is thrown before `as(..)` runs and the report lacks the description the
author wrote for exactly that case.

**How to fix.** Put `as(..)`, `describedAs(..)`, `withFailMessage(..)` or
`overridingErrorMessage(..)` before the check.

```java
// Noncompliant
assertThat(order.total()).isEqualTo(expected).as("order total");

// Compliant
assertThat(order.total()).as("order total").isEqualTo(expected);
```

Only statements whose chain starts with an AssertJ `assertThat` (statically imported from
`org.assertj`, qualified with `Assertions`, or called on a soft-assertions object) and ends
with a description right after a check (`is…`, `has…`, `contains…`, `doesNot…`, …) are
reported."#,
    ..RuleMeta::new(
        "java/assertion-description-after-check",
        "java",
        "Assertion description after the check",
    )
};

pub struct AssertionDescriptionAfterCheck;

const DESCRIBERS: &[&str] = &[
    "as",
    "describedAs",
    "withFailMessage",
    "overridingErrorMessage",
];

fn is_check(name: &str) -> bool {
    [
        "is",
        "has",
        "contains",
        "doesNot",
        "startsWith",
        "endsWith",
        "matches",
        "satisfies",
        "allMatch",
        "anyMatch",
        "noneMatch",
        "allSatisfy",
        "anySatisfy",
        "noneSatisfy",
    ]
    .iter()
    .any(|p| name.starts_with(p))
}

/// Whether the root call comes from AssertJ.
fn from_assertj<'t>(ctx: &Ctx<'t>, root: Node<'t>) -> bool {
    match root.field(Field::Object) {
        None => ctx.imports().iter().any(|i| {
            i.is_static
                && i.path.starts_with("org.assertj.")
                && (i.is_wildcard || i.simple_name() == "assertThat")
        }),
        Some(obj) => {
            let text = ctx.text(obj);
            if matches!(text, "Assertions" | "org.assertj.core.api.Assertions") {
                return ctx
                    .type_name_of(obj)
                    .is_some_and(|f| f.starts_with("org.assertj."));
            }
            kind(obj) == "identifier"
                && ctx
                    .declared_types()
                    .types_of(text)
                    .any(|t| ctx.text(t).contains("SoftAssertions"))
        }
    }
}

impl<'t> JavaRule<'t> for AssertionDescriptionAfterCheck {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        DESCRIBERS
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) = ctx.call_name(node) else {
            return;
        };
        if !DESCRIBERS.contains(&name) {
            return;
        }
        if ctx
            .parent(node)
            .is_none_or(|p| kind(p) != "expression_statement")
        {
            return;
        }
        // Down through trailing describers to the call they follow.
        let mut cur = node;
        let mut hops = 0;
        let check = loop {
            let Some(obj) = cur.field(Field::Object).map(syntax::strip_parens) else {
                return;
            };
            if kind(obj) != "method_invocation" || hops > 16 {
                return;
            }
            let Some(n) = obj.field(Field::Name).map(|x| ctx.text(x)) else {
                return;
            };
            if DESCRIBERS.contains(&n) {
                cur = obj;
                hops += 1;
                continue;
            }
            break obj;
        };
        let Some(check_name) = check.field(Field::Name).map(|x| ctx.text(x)) else {
            return;
        };
        if !is_check(check_name) {
            return;
        }
        // Down to the root `assertThat(..)`.
        let mut root = check;
        while let Some(o) = root.field(Field::Object).map(syntax::strip_parens) {
            if kind(o) != "method_invocation" || hops > 64 {
                break;
            }
            root = o;
            hops += 1;
        }
        if root.field(Field::Name).map(|x| ctx.text(x)) != Some("assertThat")
            || !from_assertj(ctx, root)
        {
            return;
        }
        let at = node.field(Field::Name).unwrap_or(node);
        ctx.emit(
            at,
            format!("`{name}(..)` comes after `{check_name}(..)` has already run: move it before the check"),
        );
    }
}
