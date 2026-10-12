//! `java/redundant-annotation-value` — `@Anno(value = x)` instead of `@Anno(x)`.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["clutter", "annotations"],
    remediation_minutes: 1,
    description: r#"Detects annotations written as `@Anno(value = x)` when `value` is the only
element given.

**Why it matters.** Java lets the element named `value` be passed without its name when it
is the only one. Spelling it out adds noise and makes the usage look different from the
common form readers expect.

**How to fix.** Drop `value =`.

```java
// Noncompliant
@SuppressWarnings(value = "unchecked")
@RequestMapping(value = "/orders")

// Compliant
@SuppressWarnings("unchecked")
@RequestMapping("/orders")
```

Annotations that set any other element next to `value` are not reported."#,
    ..RuleMeta::new(
        "java/redundant-annotation-value",
        "java",
        "Redundant `value =` in an annotation",
    )
};

pub struct RedundantAnnotationValue;

impl<'t> JavaRule<'t> for RedundantAnnotationValue {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["annotation"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(args) = util::fld(node, F.arguments) else {
            return;
        };
        let mut it = named_children(args);
        let (Some(pair), None) = (it.next(), it.next()) else {
            return;
        };
        if kind(pair) != "element_value_pair" {
            return;
        }
        let Some(key) = util::fld(pair, F.key) else {
            return;
        };
        if ctx.text(key) != "value" {
            return;
        }
        ctx.emit(
            key,
            format!(
                "Remove `value =` from `@{}`: it is the only element given",
                syntax::annotation_name(node, ctx.source())
            ),
        );
    }
}
