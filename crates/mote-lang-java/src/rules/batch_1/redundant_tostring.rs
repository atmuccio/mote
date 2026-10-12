//! `java/redundant-tostring` — `toString()` on something that is already a `String`.

use super::util::{self, Fld, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["clutter", "strings"],
    remediation_minutes: 1,
    description: r#"Detects `toString()` called on an expression whose type is already `String`.

**Why it matters.** `String.toString()` returns the string itself, so the call is noise that
suggests the author wasn't sure of the type. It also throws on `null`, which a plain reference
wouldn't.

**How to fix.** Remove the call.

```java
// Noncompliant
String name = user.getName().toString();

// Compliant
String name = user.getName();
```

Only receivers whose type is provably `String` are reported."#,
    ..RuleMeta::new(
        "java/redundant-tostring",
        "java",
        "toString() called on a String",
    )
};

pub struct RedundantToString;

impl<'t> JavaRule<'t> for RedundantToString {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["toString"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        // Text pre-filter (no tree access): the call must end in `toString()`.
        if !ctx.text(node).ends_with("toString()") || util::method_name(ctx, node) != "toString" {
            return;
        }
        let Some(recv) = util::plain_receiver(node) else {
            return;
        };
        if !ctx.type_of(recv).is_some_and(|t| t.is_string()) {
            return;
        }
        let at = node.fld(F::Name).unwrap_or(node);
        ctx.emit(at, "Remove this call; the value is already a String");
    }
}
