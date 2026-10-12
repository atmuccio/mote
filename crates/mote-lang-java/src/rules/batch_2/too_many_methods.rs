//! `java/too-many-methods` — classes declaring more methods than `max`.

use super::util::Mods;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["design", "size"],
    params: &[ParamSpec {
        name: "max",
        default: ParamDefault::Int(35),
        description: "Largest number of methods (constructors and `@Override` methods excluded) a class, enum, or record may declare.",
    }],
    remediation_minutes: 60,
    description: r#"Detects classes, enums, and records that declare more than `max` (default 35)
methods. Constructors, methods of nested types, and methods of anonymous classes are not
counted.

**Why it matters.** A type with that many methods usually has more than one responsibility.
It is hard to understand as a whole, changes for many unrelated reasons, and tends to keep
growing because it is the obvious place to add "one more" method.

**How to fix.** Identify groups of methods that work on the same subset of state or serve
the same purpose and move them into separate, focused classes (collaborators, helpers,
strategies).

```java
// Noncompliant: one class handles parsing, validation, persistence and reporting
class OrderManager { /* 60 methods */ }

// Compliant
class OrderParser { … }
class OrderValidator { … }
class OrderRepository { … }
```

Not counted: methods annotated `@Override`, whose number is dictated by the implemented
interfaces and superclasses (a JDBC wrapper must implement all of `java.sql.Statement`). Not
reported: interfaces and test files (a test class naturally has one method per test case)."#,
    ..RuleMeta::new("java/too-many-methods", "java", "Too many methods")
};

pub struct TooManyMethods;

impl<'t> JavaRule<'t> for TooManyMethods {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "class_declaration",
            "enum_declaration",
            "record_declaration",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if ctx.is_test() {
            return;
        }
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        let src = ctx.source();
        let count = ctx
            .members(body)
            .filter(|m| {
                kind(*m) == "method_declaration" && !Mods::of(*m).has_annotation(src, "Override")
            })
            .count();
        let max = ctx.param_i64("max");
        if (count as i64) <= max {
            return;
        }
        let Some(name) = node.field(Field::Name) else {
            return;
        };
        ctx.emit(
            name,
            format!(
                "Split `{}`: it declares {count} methods, more than the {max} allowed",
                ctx.text(name)
            ),
        )
        .value(count as f64);
    }
}
