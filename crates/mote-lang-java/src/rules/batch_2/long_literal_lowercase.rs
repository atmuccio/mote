//! `java/long-literal-lowercase` — `long` literals with a lowercase `l` suffix.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["readability", "convention"],
    remediation_minutes: 1,
    description: r#"Detects `long` integer literals written with a lowercase `l` suffix.

**Why it matters.** In most fonts a lowercase `l` is almost indistinguishable from the digit
`1`: `10l` easily reads as `101`. The uppercase `L` is unambiguous.

**How to fix.** Use the uppercase suffix.

```java
// Noncompliant
long timeout = 30000l;

// Compliant
long timeout = 30000L;
```"#,
    ..RuleMeta::new(
        "java/long-literal-lowercase",
        "java",
        "Long literal with lowercase suffix",
    )
};

pub struct LongLiteralLowercase;

impl<'t> JavaRule<'t> for LongLiteralLowercase {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "decimal_integer_literal",
            "hex_integer_literal",
            "octal_integer_literal",
            "binary_integer_literal",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if let Some(digits) = ctx.text(node).strip_suffix('l') {
            ctx.emit(node, format!("Use an uppercase `L` suffix: `{digits}L`"));
        }
    }
}
