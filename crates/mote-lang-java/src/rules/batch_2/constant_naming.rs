//! `java/constant-naming` — `static final` primitive/String fields follow UPPER_SNAKE.

use super::util::{binds_native_code, FormatCache, Mods};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["naming", "convention"],
    params: &[ParamSpec {
        name: "format",
        default: ParamDefault::Str("UPPER_SNAKE"),
        description: "Naming preset for constants: `UPPER_SNAKE` (`^[A-Z][A-Z0-9]*(_[A-Z0-9]+)*$`), `camelCase`, `PascalCase`, or `snake_case`. Arbitrary regular expressions are not supported; the regular expression of a preset is accepted as an alias. With an unknown value the rule reports nothing.",
    }],
    remediation_minutes: 5,
    description: r#"Detects constants whose name does not follow the configured convention (by
default UPPER_SNAKE: uppercase letters and digits, words separated by single underscores).
A constant here is a `static final` field — or an interface field, which is implicitly
`static final` — of a primitive type or `String`.

**Why it matters.** Upper-case constant names tell readers that a value is fixed for the
lifetime of the program, which is useful when reasoning about the code; mixed conventions blur
that distinction.

**How to fix.** Rename the constant.

```java
// Noncompliant
static final int maxRetries = 3;
static final String defaultName = "n/a";

// Compliant
static final int MAX_RETRIES = 3;
static final String DEFAULT_NAME = "n/a";
```

Not reported: `serialVersionUID`, logger fields, arrays, and `static final` fields of other
types (collections, loggers, mutable objects), whose naming conventions vary between teams,
and files that bind native code through JNA (names mirror native constants). The `format`
parameter takes a preset name; regular expressions are not supported."#,
    ..RuleMeta::new("java/constant-naming", "java", "Constant naming convention")
};

#[derive(Default)]
pub struct ConstantNaming {
    format: FormatCache,
}

/// Whether the declared type is a primitive or `String` (not an array).
fn is_constant_type(ty: Node<'_>, src: &str) -> bool {
    match kind(ty) {
        "integral_type" | "floating_point_type" | "boolean_type" => true,
        "type_identifier" | "scoped_type_identifier" => {
            matches!(syntax::node_text(ty, src), "String" | "java.lang.String")
        }
        _ => false,
    }
}

impl<'t> JavaRule<'t> for ConstantNaming {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["field_declaration", "constant_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if kind(node) == "field_declaration"
            && !(Mods::of(node).has("static") && Mods::of(node).has("final"))
        {
            return;
        }
        let Some(ty) = node.field(Field::Type) else {
            return;
        };
        if !is_constant_type(ty, ctx.source()) {
            return;
        }
        let Some(format) = self.format.get(ctx) else {
            return;
        };
        for d in syntax::declarators(node) {
            if d.field(Field::Dimensions).is_some() {
                continue;
            }
            let Some(name) = d.field(Field::Name) else {
                continue;
            };
            if kind(name) != "identifier" {
                continue;
            }
            let text = ctx.text(name);
            if format.matches(text)
                || text == "serialVersionUID"
                || text.eq_ignore_ascii_case("log")
                || text.eq_ignore_ascii_case("logger")
            {
                continue;
            }
            if binds_native_code(ctx) {
                return;
            }
            ctx.emit(
                name,
                format!(
                    "Rename constant `{text}` to match the {} naming convention",
                    format.label()
                ),
            );
        }
    }
}
