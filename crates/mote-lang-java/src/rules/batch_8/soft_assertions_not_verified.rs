//! `java/soft-assertions-not-verified` — an AssertJ `SoftAssertions` object whose collected
//! failures are never reported with `assertAll()`.

use super::util::{self, Walk};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["tests", "assertj"],
    remediation_minutes: 2,
    description: r#"Detects a local `SoftAssertions` (or `BDDSoftAssertions`) created with `new`
whose `assertAll()` is never called in the method.

**Why it matters.** Soft assertions do not throw when a check fails: they record the failure
and report all of them together when `assertAll()` is called. Without that call nothing is
ever reported, so every check made through the object passes, whatever the values.

**How to fix.** Call `assertAll()` at the end of the test, or let AssertJ do it: use
`SoftAssertions.assertSoftly(softly -> { … })`, an `AutoCloseableSoftAssertions` in a
try-with-resources statement, or the JUnit 5 `SoftAssertionsExtension`.

```java
// Noncompliant
SoftAssertions softly = new SoftAssertions();
softly.assertThat(user.name()).isEqualTo("Ada");
softly.assertThat(user.age()).isEqualTo(36);

// Compliant
SoftAssertions.assertSoftly(softly -> {
    softly.assertThat(user.name()).isEqualTo("Ada");
    softly.assertThat(user.age()).isEqualTo(36);
});
```

Not reported: objects whose collected failures are read instead
(`softly.assertionErrorsCollected()`, `errorsCollected()`, `wasSuccess()`: the test checks
them itself), objects passed to another method, returned, or stored in a field (the call may
happen elsewhere), and `AutoCloseableSoftAssertions`, which verifies itself when closed."#,
    ..RuleMeta::new(
        "java/soft-assertions-not-verified",
        "java",
        "Soft assertions never verified",
    )
};

pub struct SoftAssertionsNotVerified;

const SOFT_TYPES: &[&str] = &[
    "org.assertj.core.api.SoftAssertions",
    "org.assertj.core.api.BDDSoftAssertions",
];

impl<'t> JavaRule<'t> for SoftAssertionsNotVerified {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["local_variable_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let text = ctx.text(node);
        if !text.contains("SoftAssertions") || !text.contains("new ") {
            return;
        }
        for d in syntax::declarators(node) {
            let (Some(name_node), Some(value)) = (d.field(Field::Name), d.field(Field::Value))
            else {
                continue;
            };
            let value = syntax::strip_parens(value);
            if kind(value) != "object_creation_expression" || syntax::type_body(value).is_some() {
                continue;
            }
            let Some(ty) = value.field(Field::Type) else {
                continue;
            };
            let base = if kind(ty) == "generic_type" {
                syntax::first_named_child(ty)
            } else {
                Some(ty)
            };
            let Some(base) = base else { continue };
            let fqn = ctx.resolve_type_name(ctx.text(base), base);
            if !fqn.as_deref().is_some_and(|f| SOFT_TYPES.contains(&f)) {
                continue;
            }
            let Some(scope) = ctx.parent(node) else {
                continue;
            };
            let name = ctx.text(name_node);
            // Every use of the variable after its declaration must be `name.assertThat…` style;
            // any `assertAll`, or any other use of the variable, ends the search.
            let mut verified = false;
            let mut escaped = false;
            util::visit(scope, |n| {
                if n.end_byte() <= d.end_byte() {
                    return Walk::Skip;
                }
                if kind(n) != "identifier" || ctx.text(n) != name {
                    return Walk::Continue;
                }
                let parent = ctx.parent(n);
                match parent {
                    Some(p)
                        if kind(p) == "method_invocation"
                            && p.field(Field::Object).is_some_and(|o| o.id() == n.id()) =>
                    {
                        let m = p.field(Field::Name).map(|x| ctx.text(x)).unwrap_or("");
                        if matches!(
                            m,
                            "assertAll"
                                | "errorsCollected"
                                | "assertionErrorsCollected"
                                | "wasSuccess"
                        ) {
                            verified = true;
                            return Walk::Stop;
                        }
                        Walk::Continue
                    }
                    _ => {
                        escaped = true;
                        Walk::Stop
                    }
                }
            });
            if verified || escaped {
                continue;
            }
            ctx.emit(
                name_node,
                format!(
                    "`{name}` collects failures that are never reported: call `{name}.assertAll()`"
                ),
            );
        }
    }
}
