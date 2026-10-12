//! `java/class-naming` — type names follow the configured convention (PascalCase).

use super::util::{binds_native_code, FormatCache};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["naming", "convention"],
    params: &[ParamSpec {
        name: "format",
        default: ParamDefault::Str("PascalCase"),
        description: "Naming preset for type names: `PascalCase` (`^[A-Z][a-zA-Z0-9]*$`), `camelCase`, `UPPER_SNAKE`, or `snake_case`. Arbitrary regular expressions are not supported; the regular expression of a preset is accepted as an alias. With an unknown value the rule reports nothing.",
    }],
    remediation_minutes: 5,
    description: r#"Detects classes, interfaces, enums, records, and annotation types whose name
does not follow the configured convention (by default PascalCase: an uppercase letter followed
by letters and digits, no underscores).

**Why it matters.** Java code is read far more often than written; a consistent naming scheme
lets readers tell types, variables and constants apart at a glance, and it is what every Java
tool, framework, and developer expects.

**How to fix.** Rename the type (and its file, for top-level public types).

```java
// Noncompliant
class order_line { }
interface httpHandler { }

// Compliant
class OrderLine { }
interface HttpHandler { }
```

Letters are classified by Unicode case (`Äpfel` is fine), and an underscore between two digits
(a version number such as `Api2_0`) is accepted. Not reported: files that bind native code
through JNA, whose structure and callback types mirror native names (`SERVICE_STATUS`). The
`format` parameter takes a preset name (`PascalCase`, `camelCase`, `UPPER_SNAKE`,
`snake_case`); regular expressions are not supported."#,
    ..RuleMeta::new("java/class-naming", "java", "Type naming convention")
};

#[derive(Default)]
pub struct ClassNaming {
    format: FormatCache,
}

impl<'t> JavaRule<'t> for ClassNaming {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "class_declaration",
            "interface_declaration",
            "enum_declaration",
            "record_declaration",
            "annotation_type_declaration",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) = node.field(Field::Name) else {
            return;
        };
        let Some(format) = self.format.get(ctx) else {
            return;
        };
        let text = ctx.text(name);
        if text.is_empty() || format.matches(text) || binds_native_code(ctx) {
            return;
        }
        let what = match kind(node) {
            "interface_declaration" => "interface",
            "enum_declaration" => "enum",
            "record_declaration" => "record",
            "annotation_type_declaration" => "annotation type",
            _ => "class",
        };
        ctx.emit(
            name,
            format!(
                "Rename {what} `{text}` to match the {} naming convention",
                format.label()
            ),
        );
    }
}
