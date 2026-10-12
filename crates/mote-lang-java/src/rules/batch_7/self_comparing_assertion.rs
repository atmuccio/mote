//! `java/self-comparing-assertion` — `assertEquals(x, x)`, `assertSame(a, a)`,
//! `assertThat(v).isEqualTo(v)`: assertions that compare a value with itself.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["tests", "suspicious", "copy-paste"],
    remediation_minutes: 2,
    description: r#"Detects test assertions whose expected and actual values are the same
expression:

* `assertSame(a, a)` / `assertNotSame(a, a)` (any type);
* `assertEquals(x, x)`, `assertNotEquals(x, x)`, `assertArrayEquals(x, x)` when `x` is a
  primitive, a boxed number, a `String`, an enum or an array — types whose equality is fixed by
  the JDK;
* AssertJ-style `assertThat(x).isEqualTo(x)` / `isSameAs(x)` / `isNotEqualTo(x)` /
  `isNotSameAs(x)`.

**Why it matters.** The assertion cannot fail (or cannot pass), whatever the code under test
does: the test silently checks nothing. It usually comes from copying a line and forgetting to
change one side (`assertEquals(expected.getId(), expected.getId())`-style slips are common).

**How to fix.** Compare the value produced by the code under test with the expected one.

```java
// Noncompliant
assertEquals(expectedTotal, expectedTotal);

// Compliant
assertEquals(expectedTotal, invoice.total());
```

Not reported: arguments that contain calls or `new` (two calls may return different values,
which some tests check deliberately), literals, and `assertEquals(obj, obj)` on other types,
which exercises the reflexivity of a custom `equals`."#,
    ..RuleMeta::new(
        "java/self-comparing-assertion",
        "java",
        "Assertion compares a value with itself",
    )
};

pub struct SelfComparingAssertion;

/// Whether two argument nodes are the same side-effect-free, non-literal expression.
fn same<'t>(ctx: &Ctx<'t>, a: Node<'t>, b: Node<'t>) -> bool {
    let (a, b) = (syntax::strip_parens(a), syntax::strip_parens(b));
    kind(a) == kind(b)
        && matches!(kind(a), "identifier" | "field_access" | "array_access")
        && ctx.text(a) == ctx.text(b)
        && !util::has_side_effects(a)
}

/// Whether equality of values of this type is fixed by the JDK (no custom `equals`).
fn jdk_equality(ctx: &Ctx<'_>, t: &JType) -> bool {
    t.is_primitive()
        || t.is_boxed()
        || t.is_string()
        || t.is_array()
        || ctx.is_subtype_of(t, "java.lang.Enum")
}

impl<'t> JavaRule<'t> for SelfComparingAssertion {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &[
            "assertEquals",
            "assertNotEquals",
            "assertArrayEquals",
            "assertSame",
            "assertNotSame",
            "isEqualTo",
            "isNotEqualTo",
            "isSameAs",
            "isNotSameAs",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) = ctx.call_name(node) else {
            return;
        };
        if name.starts_with("is") {
            let mut args = util::args(node);
            let (Some(expected), None) = (args.next(), args.next()) else {
                return;
            };
            let Some(chain) = node.field(Field::Object) else {
                return;
            };
            if !matches!(ctx.call_name(chain), Some("assertThat" | "then")) {
                return;
            }
            let mut inner = util::args(chain);
            let (Some(actual), None) = (inner.next(), inner.next()) else {
                return;
            };
            if same(ctx, actual, expected) {
                ctx.emit(
                    node,
                    format!(
                        "Compare with a different value: `{}` is checked against itself",
                        ctx.text(actual)
                    ),
                );
            }
            return;
        }
        let mut args = util::args(node);
        let (Some(a0), Some(a1)) = (args.next(), args.next()) else {
            return;
        };
        let a2 = args.next();
        if a2.is_some() && args.nth(1).is_some() {
            return;
        }
        // (expected, actual) is (0, 1), or (1, 2) after a leading message.
        let pair = if same(ctx, a0, a1) {
            Some(a0)
        } else {
            a2.filter(|a2| same(ctx, a1, *a2)).map(|_| a1)
        };
        let Some(value) = pair else {
            return;
        };
        let identity = matches!(name, "assertSame" | "assertNotSame");
        if !identity && !ctx.type_of(value).is_some_and(|t| jdk_equality(ctx, &t)) {
            return;
        }
        ctx.emit(
            node,
            format!(
                "Compare with a different value: `{}` is checked against itself",
                ctx.text(value)
            ),
        );
    }
}
