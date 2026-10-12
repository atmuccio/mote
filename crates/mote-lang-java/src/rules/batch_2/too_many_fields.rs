//! `java/too-many-fields` — classes declaring more non-constant fields than `max`.

use super::util::Mods;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["design", "size"],
    params: &[ParamSpec {
        name: "max",
        default: ParamDefault::Int(20),
        description: "Largest number of non-constant fields (fields that are not `static final`) a class, enum, or record may declare.",
    }],
    remediation_minutes: 60,
    description: r#"Detects classes, enums, and records that declare more than `max` (default 20)
fields that are not `static final`. Each variable of a multi-variable declaration
(`int a, b;`) counts.

**Why it matters.** Many fields mean much state to keep consistent. Such classes usually
mix several concepts, are hard to construct and test, and every method potentially depends on
every field.

**How to fix.** Group related fields into small value classes or records (an `Address`
instead of `street`, `city`, `zip`, …) and move behaviour that uses only one group along with
it.

```java
// Noncompliant
class Customer {
    String street; String city; String zip; String country;
    String phone; String mobile; String email; /* … 20 more … */
}

// Compliant
class Customer {
    Address address;
    ContactInfo contact;
}
```

Constants (`static final` fields) are not counted."#,
    ..RuleMeta::new("java/too-many-fields", "java", "Too many fields")
};

pub struct TooManyFields;

impl<'t> JavaRule<'t> for TooManyFields {
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
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        let count: usize = ctx
            .members(body)
            .filter(|m| {
                kind(*m) == "field_declaration"
                    && !(Mods::of(*m).has("static") && Mods::of(*m).has("final"))
            })
            .map(|m| syntax::declarators(m).count())
            .sum();
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
                "Reduce the number of fields of `{}`: it declares {count}, more than the {max} allowed",
                ctx.text(name)
            ),
        )
        .value(count as f64);
    }
}
