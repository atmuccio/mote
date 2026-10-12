//! `java/disabled-test-without-reason` — `@Disabled` / `@Ignore` without an explanation.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["tests"],
    remediation_minutes: 2,
    description: r#"Detects JUnit `@Disabled` / `@Ignore` (and TestNG `@Ignore`) annotations that
don't give a reason.

**Why it matters.** A disabled test silently stops protecting the code. Without a reason
nobody knows whether it is flaky, waiting for a fix, or obsolete, so it tends to stay disabled
forever. Test reports show the reason string next to the skipped test.

**How to fix.** Give the reason (ideally with an issue reference), or delete the test if it is
no longer relevant.

```java
// Noncompliant
@Disabled
@Test
void importsLargeFile() { … }

// Compliant
@Disabled("Flaky on Windows CI, see #1234")
@Test
void importsLargeFile() { … }
```

Only annotations that resolve to JUnit 4 (`org.junit.Ignore`), JUnit 5
(`org.junit.jupiter.api.Disabled`), or TestNG (`org.testng.annotations.Ignore`) through the
file's imports are checked; an empty or blank string counts as no reason."#,
    ..RuleMeta::new(
        "java/disabled-test-without-reason",
        "java",
        "Disabled test without a reason",
    )
};

pub struct DisabledTestWithoutReason;

impl<'t> JavaRule<'t> for DisabledTestWithoutReason {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["marker_annotation", "annotation"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let name = syntax::annotation_name(node, ctx.source());
        if !matches!(name, "Disabled" | "Ignore") {
            return;
        }
        if !util::annotation_is(ctx, node, util::DISABLE_ANNOTATIONS) {
            return;
        }
        if kind(node) == "annotation" && has_reason(node, ctx) {
            return;
        }
        ctx.emit(
            node,
            format!("Explain in `@{name}(\"…\")` why this test is disabled"),
        );
    }
}

/// Whether the annotation's arguments carry a non-blank string or a constant reference.
fn has_reason<'t>(ann: Node<'t>, ctx: &Ctx<'t>) -> bool {
    let Some(args) = ann.field(Field::Arguments) else {
        return false;
    };
    named_children(args).any(|a| {
        let value = if kind(a) == "element_value_pair" {
            match a.field(Field::Value) {
                Some(v) => v,
                None => return false,
            }
        } else {
            a
        };
        is_reason(value, ctx)
    })
}

fn is_reason<'t>(value: Node<'t>, ctx: &Ctx<'t>) -> bool {
    let value = syntax::strip_parens(value);
    match kind(value) {
        "string_literal" => syntax::string_literal_content(value, ctx.source())
            .is_some_and(|s| !s.trim().is_empty()),
        // `"a" + "b"`, a constant, or anything else that isn't obviously empty.
        "binary_expression" | "identifier" | "field_access" | "method_invocation" => true,
        _ => false,
    }
}
