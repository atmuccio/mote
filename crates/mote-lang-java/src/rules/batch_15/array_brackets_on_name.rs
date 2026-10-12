//! `java/array-brackets-on-name` — `String args[]` or `int values()[]`: array brackets written
//! after the name instead of on the type.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["conventions", "arrays"],
    remediation_minutes: 1,
    description: r#"Detects array brackets placed after a variable, parameter or field name
(`String args[]`) or after a method's parameter list (`int values()[]`) instead of on the type.

**Why it matters.** The brackets are part of the type. Written after the name, they split the
type in two: a reader scanning the left column sees `String` where the variable is a
`String[]`, and `int[] grid[]` is a two-dimensional array whose dimensions sit on both sides of
the name. The trailing form exists for compatibility with C; Java code reads more easily when
the whole type is written in one place.

**How to fix.** Move the brackets to the type.

```java
// Noncompliant
public static void main(String args[]) { … }
private int counts[];
int values()[] { … }

// Compliant
public static void main(String[] args) { … }
private int[] counts;
int[] values() { … }
```

Covered: fields, local variables, parameters (record components included), methods and
annotation elements. The rule is off by default: it is part of the `conventions` profile."#,
    ..RuleMeta::new(
        "java/array-brackets-on-name",
        "java",
        "Array brackets after the name instead of on the type",
    )
};

pub struct ArrayBracketsOnName;

impl<'t> JavaRule<'t> for ArrayBracketsOnName {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        // `dimensions` nodes are rare (array types and these declarations): far fewer calls
        // than subscribing to every declarator.
        &["dimensions"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(decl) = ctx.parent(node) else {
            return;
        };
        let on_return_type = match kind(decl) {
            "variable_declarator" | "formal_parameter" => false,
            "method_declaration" | "annotation_type_element_declaration" => true,
            _ => return,
        };
        if decl.field(Field::Dimensions) != Some(node) {
            return;
        }
        let name = ctx.name_of(decl);
        let message = if on_return_type {
            format!("Move `{}` to the return type of `{name}`", ctx.text(node))
        } else {
            format!(
                "Move `{}` from the name `{name}` to its type",
                ctx.text(node)
            )
        };
        ctx.emit(node, message);
    }
}
