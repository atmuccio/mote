//! `java/instanceof-pattern-candidate` — `instanceof` followed by a cast of the same variable.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["java16", "clarity"],
    status: RuleStatus::Stable,
    remediation_minutes: 2,
    description: r#"Detects `if (x instanceof T)` whose then-branch casts the same variable to
the same type, `(T) x`.

**Why it matters.** Since Java 16, pattern matching for `instanceof` binds a typed variable in
the test itself. The separate cast repeats the type, can drift out of sync with the check
during refactoring, and adds noise.

**How to fix.** Declare a pattern variable in the `instanceof` and use it instead of the cast.

```java
// Noncompliant
if (shape instanceof Circle) {
    Circle c = (Circle) shape;
    return c.radius();
}

// Compliant
if (shape instanceof Circle c) {
    return c.radius();
}
```

Only reported when the file is known to be compiled for Java 16 or later: `java.release` in
`.mote.yaml`, or the release declared by the nearest `pom.xml` / `build.gradle(.kts)`
(`maven.compiler.release`, `sourceCompatibility`, …). When the release is unknown or older,
the rule stays silent — the suggested syntax would not compile. The variable must be a plain
name (or `this.name`), the `instanceof` must be the condition or a `&&` operand of the
condition, and the cast must use exactly the same type text."#,
    ..RuleMeta::new(
        "java/instanceof-pattern-candidate",
        "java",
        "instanceof followed by a cast",
    )
};

pub struct InstanceofPatternCandidate;

impl<'t> JavaRule<'t> for InstanceofPatternCandidate {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["instanceof_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        // Pattern matching for `instanceof` is final in Java 16.
        if ctx.java_release().is_none_or(|r| r < 16) {
            return;
        }
        if util::name_field(node).is_some() || node.field(Field::Pattern).is_some() {
            return;
        }
        let (Some(left), Some(right)) = (
            node.field(Field::Left).map(syntax::strip_parens),
            node.field(Field::Right),
        ) else {
            return;
        };
        if util::simple_or_this_field(left).is_none() {
            return;
        }
        // Climb through parentheses and `&&` to the condition of an `if`.
        let mut cur = node;
        let mut if_stmt = None;
        for a in ctx.ancestors(node) {
            match kind(a) {
                "parenthesized_expression" => {}
                "binary_expression" if syntax::operator_str(a) == "&&" => {}
                "if_statement" => {
                    if a.field(Field::Condition)
                        .is_some_and(|c| c.id() == cur.id())
                    {
                        if_stmt = Some(a);
                    }
                    break;
                }
                _ => break,
            }
            cur = a;
        }
        let Some(then) = if_stmt.and_then(|s| s.field(Field::Consequence)) else {
            return;
        };
        let var = ctx.text(left);
        let ty = ctx.text(right);
        let is_cast = |n: Node<'t>| {
            kind(n) == "cast_expression"
                && n.field(Field::Type).is_some_and(|t| ctx.text(t) == ty)
                && n.field(Field::Value)
                    .is_some_and(|v| ctx.text(syntax::strip_parens(v)) == var)
        };
        let casts =
            is_cast(then) || util::any_descendant(then, |n| kind(n) == "class_body", is_cast);
        if casts {
            ctx.emit(
                node,
                format!("Use a pattern variable (`{var} instanceof {ty} name`) instead of casting `{var}` again"),
            );
        }
    }
}
