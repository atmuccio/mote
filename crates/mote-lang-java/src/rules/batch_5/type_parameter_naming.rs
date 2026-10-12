//! `java/type-parameter-naming` — type parameters should be a single uppercase letter.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["naming", "convention"],
    remediation_minutes: 2,
    description: r#"Detects type parameters whose name is not a single uppercase letter,
optionally followed by one digit (`T`, `E`, `K`, `V`, `R`, `T2`).

**Why it matters.** The one-letter convention is what makes type variables stand out from
class names in a signature: in `Result<Entity> load(Class<Entity> type)` the reader has to
check whether `Entity` is a real class or a placeholder, while `<E> Result<E> load(Class<E>
type)` is unambiguous. Mixed conventions also make generic code harder to scan.

**How to fix.** Rename the parameter to a meaningful single letter (`E` for elements, `K`/`V`
for keys and values, `T` for a general type, `R` for a result) and describe it in the Javadoc
`@param <T>` tag when more detail is useful.

```java
// Noncompliant
class Cache<KeyType, ValueType> { ... }
<Item> List<Item> copy(List<Item> items) { ... }

// Compliant
class Cache<K, V> { ... }
<E> List<E> copy(List<E> items) { ... }
```"#,
    ..RuleMeta::new(
        "java/type-parameter-naming",
        "java",
        "Type parameter naming",
    )
};

pub struct TypeParameterNaming;

fn follows_convention(name: &str) -> bool {
    let b = name.as_bytes();
    match b {
        [c] => c.is_ascii_uppercase(),
        [c, d] => c.is_ascii_uppercase() && d.is_ascii_digit(),
        _ => false,
    }
}

impl<'t> JavaRule<'t> for TypeParameterNaming {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["type_parameter"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) = named_children(node).find(|c| kind(*c) == "type_identifier") else {
            return;
        };
        let text = ctx.text(name);
        if follows_convention(text) {
            return;
        }
        ctx.emit(
            name,
            format!(
                "Rename type parameter `{text}` to a single uppercase letter, optionally followed \
                 by a digit"
            ),
        );
    }
}
