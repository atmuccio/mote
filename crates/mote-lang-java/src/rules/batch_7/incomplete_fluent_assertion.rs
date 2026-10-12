//! `java/incomplete-fluent-assertion` — `assertThat(x);` with no check chained after it.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Stable,
    tags: &["tests", "assertj"],
    remediation_minutes: 2,
    description: r#"Detects fluent assertions (AssertJ, Truth, FEST) that are started but never
finished: a statement that is only `assertThat(actual)`, possibly followed by description
calls (`.as(..)`, `.describedAs(..)`, `.withFailMessage(..)`) or by calls that only
configure or transform what is checked (`.usingRecursiveComparison()`, `.usingComparator(..)`,
`.extracting(..)`), and the same for `assertThatCode(..)` and `assertThatExceptionOfType(..)`
without their check. Besides statements, the expression body of a lambda handed to a method
that discards its result (`satisfies(e -> assertThat(e.id()))`, `allSatisfy`, `forEach`, …)
is checked too.

**Why it matters.** In fluent libraries `assertThat(x)` only creates an assertion object;
nothing is verified until a check such as `isEqualTo(..)` or `isTrue()` is called on it. The
line looks like an assertion, passes unconditionally, and the test verifies less than it seems.

**How to fix.** Chain the intended check.

```java
// Noncompliant
assertThat(result.isValid());
assertThat(order.total()).as("total");

// Compliant
assertThat(result.isValid()).isTrue();
assertThat(order.total()).as("total").isEqualTo(expected);
```

Only calls that provably come from a fluent assertion library are reported: qualified by one
of its entry classes (`Assertions`, `Truth`, …), statically imported from `org.assertj`,
`com.google.common.truth` or `org.fest`, made on a `SoftAssertions`-typed variable, or in a
class implementing AssertJ's `WithAssertions`. Hamcrest's two-argument `assertThat(x, matcher)`
is complete and never reported; a one-argument `assertThat` helper declared in the file is
skipped too."#,
    ..RuleMeta::new(
        "java/incomplete-fluent-assertion",
        "java",
        "Fluent assertion without a check",
    )
};

pub struct IncompleteFluentAssertion;

const DESCRIBERS: &[&str] = &[
    "as",
    "describedAs",
    "withFailMessage",
    "overridingErrorMessage",
];

/// Calls that return a new assertion object to check (configuration or navigation): a chain
/// ending in them has not checked anything yet.
const CONFIGURERS: &[&str] = &["usingRecursiveComparison", "usingComparator", "extracting"];

/// Methods taking a `Consumer`-like lambda whose result is discarded: an expression body
/// `e -> assertThat(e.id())` is a statement in disguise.
const CONSUMERS: &[&str] = &[
    "satisfies",
    "allSatisfy",
    "anySatisfy",
    "noneSatisfy",
    "satisfiesExactly",
    "satisfiesExactlyInAnyOrder",
    "satisfiesOnlyOnce",
    "hasOnlyOneElementSatisfying",
    "zipSatisfy",
    "forEach",
    "forEachOrdered",
    "ifPresent",
];

const ENTRY_CLASSES: &[&str] = &[
    "Assertions",
    "AssertionsForClassTypes",
    "AssertionsForInterfaceTypes",
    "Java6Assertions",
    "Truth",
    "Truth8",
    "org.assertj.core.api.Assertions",
    "com.google.common.truth.Truth",
];

const LIBRARY_PACKAGES: &[&str] = &["org.assertj.", "com.google.common.truth.", "org.fest."];

/// Whether the root call's method comes from a fluent assertion library.
fn from_fluent_library<'t>(ctx: &Ctx<'t>, root: Node<'t>, name: &str) -> bool {
    match root.field(Field::Object) {
        Some(obj) => {
            let text = ctx.text(obj);
            if ENTRY_CLASSES.contains(&text) {
                // A project class of the same name (with its own `assertThat`) must not count.
                return ctx
                    .type_name_of(obj)
                    .is_none_or(|f| LIBRARY_PACKAGES.iter().any(|p| f.starts_with(p)));
            }
            kind(obj) == "identifier"
                && ctx
                    .declared_types()
                    .types_of(text)
                    .any(|t| ctx.text(t).contains("SoftAssertions"))
        }
        None => {
            if ctx
                .enclosing_class(root)
                .and_then(syntax::type_body)
                .is_some_and(|b| util::declares_method(ctx, b, name, 1))
            {
                return false;
            }
            let imported = ctx.imports().iter().any(|i| {
                i.is_static
                    && LIBRARY_PACKAGES.iter().any(|p| i.path.starts_with(p))
                    && (i.is_wildcard || i.simple_name() == name)
            });
            imported
                || ctx.enclosing_class(root).is_some_and(|c| {
                    c.field(Field::Interfaces)
                        .is_some_and(|i| ctx.text(i).contains("WithAssertions"))
                })
        }
    }
}

/// Whether the call's value is thrown away: it is an expression statement, or the expression
/// body of a lambda passed to a method that ignores the lambda's result
/// (`satisfies(e -> assertThat(e.id()))`).
fn result_discarded<'t>(ctx: &Ctx<'t>, call: Node<'t>) -> bool {
    let mut ancestors = ctx.ancestors(call);
    let Some(parent) = ancestors.next() else {
        return false;
    };
    match kind(parent) {
        "expression_statement" => true,
        "lambda_expression" => {
            parent
                .field(Field::Body)
                .is_some_and(|b| b.id() == call.id())
                && ancestors.next().is_some_and(|a| kind(a) == "argument_list")
                && ancestors
                    .next()
                    .is_some_and(|m| ctx.call_name(m).is_some_and(|n| CONSUMERS.contains(&n)))
        }
        _ => false,
    }
}

impl<'t> JavaRule<'t> for IncompleteFluentAssertion {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &[
            "assertThat",
            "assertThatObject",
            "assertThatCode",
            "assertThatExceptionOfType",
            "as",
            "describedAs",
            "withFailMessage",
            "overridingErrorMessage",
            "usingRecursiveComparison",
            "usingComparator",
            "extracting",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if ctx.call_name(node).is_none() {
            return;
        }
        if !result_discarded(ctx, node) {
            return;
        }
        // Walk down through description calls to the chain's root.
        let mut root = node;
        let mut hops = 0;
        loop {
            let Some(name) = ctx.call_name(root) else {
                return;
            };
            if !DESCRIBERS.contains(&name) && !CONFIGURERS.contains(&name) {
                break;
            }
            let Some(obj) = root.field(Field::Object) else {
                return;
            };
            let obj = syntax::strip_parens(obj);
            if kind(obj) != "method_invocation" || hops > 8 {
                return;
            }
            root = obj;
            hops += 1;
        }
        let Some(name) = ctx.call_name(root) else {
            return;
        };
        let expected_args = match name {
            "assertThat" | "assertThatObject" | "assertThatCode" | "assertThatExceptionOfType" => 1,
            _ => return,
        };
        if util::arg_count(root) != expected_args {
            return;
        }
        if !from_fluent_library(ctx, root, name) {
            return;
        }
        let check = match name {
            "assertThatCode" => "`.doesNotThrowAnyException()`",
            "assertThatExceptionOfType" => "`.isThrownBy(..)`",
            _ => "a check such as `.isEqualTo(..)` or `.isTrue()`",
        };
        ctx.emit(
            root.field(Field::Name).unwrap_or(root),
            format!("Chain {check}: `{name}(..)` alone verifies nothing"),
        );
    }
}
