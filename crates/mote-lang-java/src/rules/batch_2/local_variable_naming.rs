//! `java/local-variable-naming` — locals and parameters follow camelCase.

use super::util::{FormatCache, NameFormat};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["naming", "convention"],
    params: &[ParamSpec {
        name: "format",
        default: ParamDefault::Str("camelCase"),
        description: "Naming preset for local variables and parameters: `camelCase` (`^[a-z][a-zA-Z0-9]*$`), `PascalCase`, `UPPER_SNAKE`, or `snake_case`. Arbitrary regular expressions are not supported; the regular expression of a preset is accepted as an alias. With an unknown value the rule reports nothing.",
    }],
    remediation_minutes: 2,
    description: r#"Detects local variables and parameters whose name does not follow the
configured convention (by default camelCase: a lowercase letter followed by letters and
digits). Checked: local variables, method/constructor/lambda/catch parameters, record
components, enhanced-`for` variables, try-with-resources resources, and pattern variables.

**Why it matters.** Local names are the most frequently read identifiers in a method. Mixing
conventions (`user_id`, `UserName`, `MAX`) makes it harder to tell locals from constants,
types and fields.

**How to fix.** Rename the variable.

```java
// Noncompliant
int Item_Count = items.size();
for (String Name : names) { … }

// Compliant
int itemCount = items.size();
for (String name : names) { … }
```

Single-character names (`i`, `x`, `T`) and the unnamed variable `_` are always accepted. The
`format` parameter takes a preset name; regular expressions are not supported."#,
    ..RuleMeta::new(
        "java/local-variable-naming",
        "java",
        "Local variable and parameter naming convention",
    )
};

#[derive(Default)]
pub struct LocalVariableNaming {
    format: FormatCache,
}

fn check_name<'t>(name: Node<'t>, what: &str, format: NameFormat, ctx: &Ctx<'t>) {
    if kind(name) != "identifier" {
        return;
    }
    let text = ctx.text(name);
    if text.chars().count() <= 1 || format.matches(text) {
        return;
    }
    ctx.emit(
        name,
        format!(
            "Rename {what} `{text}` to match the {} naming convention",
            format.label()
        ),
    );
}

/// The last `identifier` child (the variable of a type or record-pattern component).
fn last_identifier(node: Node<'_>) -> Option<Node<'_>> {
    named_children(node)
        .filter(|c| kind(*c) == "identifier")
        .last()
}

impl<'t> JavaRule<'t> for LocalVariableNaming {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "local_variable_declaration",
            "formal_parameter",
            "spread_parameter",
            "catch_formal_parameter",
            "enhanced_for_statement",
            "resource",
            "lambda_expression",
            "instanceof_expression",
            "type_pattern",
            "record_pattern_component",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(format) = self.format.get(ctx) else {
            return;
        };
        match kind(node) {
            "local_variable_declaration" => {
                for d in syntax::declarators(node) {
                    if let Some(n) = d.field(Field::Name) {
                        check_name(n, "local variable", format, ctx);
                    }
                }
            }
            "formal_parameter" => {
                let Some(n) = node.field(Field::Name) else {
                    return;
                };
                let in_record = ctx
                    .ancestors(node)
                    .nth(1)
                    .is_some_and(|gp| kind(gp) == "record_declaration");
                let what = if in_record {
                    "record component"
                } else {
                    "parameter"
                };
                check_name(n, what, format, ctx);
            }
            "spread_parameter" => {
                if let Some(n) = syntax::declarators(node)
                    .next()
                    .and_then(|d| d.field(Field::Name))
                {
                    check_name(n, "parameter", format, ctx);
                }
            }
            "catch_formal_parameter" => {
                if let Some(n) = node.field(Field::Name) {
                    check_name(n, "parameter", format, ctx);
                }
            }
            "enhanced_for_statement" | "resource" | "instanceof_expression" => {
                if let Some(n) = node.field(Field::Name) {
                    check_name(n, "local variable", format, ctx);
                }
            }
            "type_pattern" | "record_pattern_component" => {
                if let Some(n) = last_identifier(node) {
                    check_name(n, "local variable", format, ctx);
                }
            }
            "lambda_expression" => {
                let Some(params) = node.field(Field::Parameters) else {
                    return;
                };
                match kind(params) {
                    "identifier" => check_name(params, "parameter", format, ctx),
                    "inferred_parameters" => {
                        for p in named_children(params) {
                            check_name(p, "parameter", format, ctx);
                        }
                    }
                    // `formal_parameters` are visited as `formal_parameter` nodes.
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
