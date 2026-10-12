//! `java/package-naming` — package names are lowercase, dot-separated.

use super::util::FormatCache;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["naming", "convention"],
    params: &[ParamSpec {
        name: "format",
        default: ParamDefault::Str("lower.dotted"),
        description: r"Naming preset for package names: `lower.dotted` (`^[a-z_][a-z0-9_]*(\.[a-z_][a-z0-9_]*)*$`). Arbitrary regular expressions are not supported; the regular expression of the preset is accepted as an alias. With an unknown value the rule reports nothing.",
    }],
    remediation_minutes: 10,
    description: r#"Detects `package` declarations whose name is not all lowercase: each
dot-separated segment must start with a lowercase letter or underscore and contain only
lowercase letters, digits, and underscores.

**Why it matters.** Package names map to directories; mixed case causes trouble on
case-insensitive file systems and can collide with class names (`com.acme.Util` the package
vs. `com.acme.Util` the class). Lowercase packages are the universal Java convention.

**How to fix.** Rename the package (and move its directory).

```java
// Noncompliant
package com.acme.OrderService;

// Compliant
package com.acme.orderservice;
```

The `format` parameter takes a preset name (`lower.dotted`); regular expressions are not
supported."#,
    ..RuleMeta::new("java/package-naming", "java", "Package naming convention")
};

#[derive(Default)]
pub struct PackageNaming {
    format: FormatCache,
}

impl<'t> JavaRule<'t> for PackageNaming {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["package_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) =
            named_children(node).find(|c| matches!(kind(*c), "scoped_identifier" | "identifier"))
        else {
            return;
        };
        let Some(format) = self.format.get(ctx) else {
            return;
        };
        let text = ctx.text(name);
        // Whitespace or comments inside the dotted name (`com . acme`): too unusual to judge.
        if text.bytes().any(|b| b.is_ascii_whitespace() || b == b'/') || format.matches(text) {
            return;
        }
        ctx.emit(
            name,
            format!(
                "Rename package `{text}` to match the {} naming convention",
                format.label()
            ),
        );
    }
}
