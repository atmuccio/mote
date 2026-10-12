//! `java/field-naming` — non-constant fields follow camelCase.

use super::util::{binds_native_code, FormatCache, Mods};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["naming", "convention"],
    params: &[ParamSpec {
        name: "format",
        default: ParamDefault::Str("camelCase"),
        description: "Naming preset for fields: `camelCase` (`^[a-z][a-zA-Z0-9]*$`), `PascalCase`, `UPPER_SNAKE`, or `snake_case`. Arbitrary regular expressions are not supported; the regular expression of a preset is accepted as an alias. With an unknown value the rule reports nothing.",
    }],
    remediation_minutes: 5,
    description: r#"Detects fields that are not `static final` and whose name does not follow the
configured convention (by default camelCase: a lowercase letter followed by letters and digits,
no underscores).

**Why it matters.** Consistent field names make it obvious what is state and what is a
constant or a type. Prefixes and underscores (`m_count`, `_name`) or upper-case names on
mutable fields mislead readers.

**How to fix.** Rename the field. If it really is a constant, make it `static final`.

```java
// Noncompliant
private int Item_Count;
private String _name;

// Compliant
private int itemCount;
private String name;
```

Not reported: `static final` fields of any type (constants are covered by
`java/constant-naming`; the naming of `static final` loggers and collections varies between
teams), interface fields, record components, and fields in files that bind native code
through JNA (structure fields mirror native names). The `format` parameter takes a preset
name; regular expressions are not supported."#,
    ..RuleMeta::new("java/field-naming", "java", "Field naming convention")
};

#[derive(Default)]
pub struct FieldNaming {
    format: FormatCache,
}

impl<'t> JavaRule<'t> for FieldNaming {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["field_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if Mods::of(node).has("static") && Mods::of(node).has("final") {
            return;
        }
        let Some(format) = self.format.get(ctx) else {
            return;
        };
        for d in syntax::declarators(node) {
            let Some(name) = d.field(Field::Name) else {
                continue;
            };
            if kind(name) != "identifier" {
                continue;
            }
            let text = ctx.text(name);
            if format.matches(text) || text == "serialVersionUID" {
                continue;
            }
            if binds_native_code(ctx) {
                return;
            }
            ctx.emit(
                name,
                format!(
                    "Rename field `{text}` to match the {} naming convention",
                    format.label()
                ),
            );
        }
    }
}
