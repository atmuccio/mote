//! `java/assert-boolean-literal` — `assertEquals(true, x)` instead of `assertTrue(x)`.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["tests", "clarity"],
    remediation_minutes: 1,
    description: r#"Detects `assertEquals` calls where one of the compared values is the literal
`true` or `false`.

**Why it matters.** `assertTrue(x)` / `assertFalse(x)` say the same thing more directly, and
avoid the common mix-up of expected and actual arguments. The dedicated assertion also reads
better in failure messages.

**How to fix.** Replace the call with `assertTrue(…)` or `assertFalse(…)` (keeping the message
argument, if any).

```java
// Noncompliant
assertEquals(true, cache.isEmpty());
assertEquals("must be closed", false, conn.isOpen());

// Compliant
assertTrue(cache.isEmpty());
assertFalse("must be closed", conn.isOpen());
```

Covers JUnit 4 (message first), JUnit 5 (message last), and TestNG argument orders. Not
reported when the file declares its own `assertEquals` helper."#,
    ..RuleMeta::new(
        "java/assert-boolean-literal",
        "java",
        "assertEquals with a boolean literal",
    )
};

pub struct AssertBooleanLiteral;

impl<'t> JavaRule<'t> for AssertBooleanLiteral {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["assertEquals"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) = util::name_field(node) else {
            return;
        };
        if ctx.text(name) != "assertEquals" {
            return;
        }
        let n = util::arg_count(node);
        if !(2..=3).contains(&n) {
            return;
        }
        let Some(literal) = syntax::arguments(node).find_map(|a| {
            let a = syntax::strip_parens(a);
            match kind(a) {
                "true" => Some(true),
                "false" => Some(false),
                _ => None,
            }
        }) else {
            return;
        };
        if util::object_field(node).is_none() {
            if let Some(body) = ctx.enclosing_class(node).and_then(syntax::type_body) {
                if util::declares_method(ctx, body, "assertEquals") {
                    return;
                }
            }
        }
        let replacement = if literal { "assertTrue" } else { "assertFalse" };
        ctx.emit(
            name,
            format!("Use `{replacement}(…)` instead of comparing with `{literal}`"),
        );
    }
}
