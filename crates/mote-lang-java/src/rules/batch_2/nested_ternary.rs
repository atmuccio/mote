//! `java/nested-ternary` — a conditional expression inside another one.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["readability"],
    remediation_minutes: 5,
    description: r#"Detects a conditional (ternary) expression nested inside another conditional
expression — in its condition, in one of its branches, or in an argument of a call there.

**Why it matters.** One `?:` reads well; two or more force readers to work out which `:`
belongs to which `?`, and the precedence is easy to get wrong when editing.

**How to fix.** Use `if`/`else` statements, a `switch` expression, or extract the inner
expression into a well-named local variable or method.

```java
// Noncompliant
String size = n > 100 ? "large" : n > 10 ? "medium" : "small";

// Compliant
String size;
if (n > 100) {
    size = "large";
} else if (n > 10) {
    size = "medium";
} else {
    size = "small";
}
```

Each nested conditional is reported once. A conditional inside a lambda or an anonymous
class body that is itself inside a conditional is not considered nested."#,
    ..RuleMeta::new(
        "java/nested-ternary",
        "java",
        "Nested conditional expression",
    )
};

pub struct NestedTernary;

impl<'t> JavaRule<'t> for NestedTernary {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["ternary_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        for a in ctx.ancestors(node) {
            match kind(a) {
                "ternary_expression" => {
                    ctx.emit(
                        node,
                        "Extract this nested conditional expression into a variable, a method, or if/else statements",
                    );
                    return;
                }
                "lambda_expression" | "class_body" | "block" | "switch_block" | "program" => {
                    return;
                }
                k if k.ends_with("_statement") || k.ends_with("_declaration") => return,
                _ => {}
            }
        }
    }
}
