//! `java/method-naming` — method names follow the configured convention (camelCase).

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
        description: "Naming preset for method names: `camelCase` (`^[a-z][a-zA-Z0-9]*$`), `PascalCase`, `UPPER_SNAKE`, or `snake_case`. Arbitrary regular expressions are not supported; the regular expression of a preset is accepted as an alias. With an unknown value the rule reports nothing.",
    }],
    remediation_minutes: 5,
    description: r#"Detects methods whose name does not follow the configured convention (by
default camelCase: a lowercase letter followed by letters and digits).

**Why it matters.** Consistent names make code predictable: in Java, a lowercase identifier
followed by `(` is a method call, an uppercase one is a type. Deviations slow readers down and
look like mistakes.

**How to fix.** Rename the method.

```java
// Noncompliant
public int Get_Count() { … }

// Compliant
public int getCount() { … }
```

Not reported: constructors, methods annotated `@Override` (the name is dictated by the
supertype), `native` methods, and methods in files that bind native code through JNA (the
names mirror native functions such as `ShellExecuteEx`). In test files underscores are
accepted (`rejects_empty_input`, `Parser_emptyInput_throws`). An underscore between two digits
(a version number such as `isBefore3_8`) is always accepted. The `format` parameter takes a
preset name; regular expressions are not supported."#,
    ..RuleMeta::new("java/method-naming", "java", "Method naming convention")
};

#[derive(Default)]
pub struct MethodNaming {
    format: FormatCache,
}

/// `Unit_scenario_expectation` test names, whose first segment names the (capitalized) class
/// under test: letters, digits, and underscores, starting with a letter, with an underscore.
fn is_scenario_test_name(name: &str) -> bool {
    name.contains('_')
        && name.chars().next().is_some_and(char::is_alphabetic)
        && name.chars().all(|c| c.is_alphanumeric() || c == '_')
}

impl<'t> JavaRule<'t> for MethodNaming {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) = ctx.name_node(node) else {
            return;
        };
        let text = ctx.text(name);
        let Some(format) = self.format.get(ctx) else {
            return;
        };
        let ok = if ctx.is_test() {
            format.matches_allowing_underscores(text) || is_scenario_test_name(text)
        } else {
            format.matches(text)
        };
        if ok || text.is_empty() {
            return;
        }
        if Mods::of(node).has("native")
            || Mods::of(node).has_annotation(ctx.source(), "Override")
            || binds_native_code(ctx)
        {
            return;
        }
        ctx.emit(
            name,
            format!(
                "Rename method `{text}` to match the {} naming convention",
                format.label()
            ),
        );
    }
}
