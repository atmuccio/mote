//! `java/overloads-not-adjacent` — methods sharing a name, or the constructors of a type,
//! separated by other members.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["conventions", "structure"],
    remediation_minutes: 2,
    description: r#"Detects overloads that are not kept together: a method with the same name
as an earlier method of the same type, or a constructor, with other members in between.

**Why it matters.** Overloads are variations of one operation, and a reader choosing between
them, or changing their shared behavior, needs to see all of them at once. Scattered through a
long class, the third `parse` is missed when the first two are fixed, and a reviewer cannot
tell whether the overloads still agree with each other.

**How to fix.** Move the overload next to the others with that name (or the constructor next
to the other constructors), with no other member in between.

```java
// Noncompliant
class Parser {
    Node parse(String text) { … }
    void reset() { … }
    Node parse(Reader in) { … }
}

// Compliant
class Parser {
    Node parse(String text) { … }
    Node parse(Reader in) { … }
    void reset() { … }
}
```

Every member counts as a separator: fields, nested types, initializers and other methods.
Each overload that does not follow the previous one of its name is reported, with the
previous one as secondary location. The rule is off by default: it is part of the
`conventions` profile."#,
    ..RuleMeta::new(
        "java/overloads-not-adjacent",
        "java",
        "Overloads separated by other members",
    )
};

pub struct OverloadsNotAdjacent;

impl<'t> JavaRule<'t> for OverloadsNotAdjacent {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["class_body", "interface_body", "enum_body"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        // Name → (index of its latest occurrence, that member); constructors under "". (A map:
        // searching a list per member was quadratic in a class of many methods.)
        let mut last: FxHashMap<&'t str, (usize, Node<'t>)> = FxHashMap::default();
        for (i, m) in ctx.members(node).enumerate() {
            let key = match kind(m) {
                "method_declaration" => ctx.name_of(m),
                "constructor_declaration" | "compact_constructor_declaration" => "",
                _ => continue,
            };
            match last.get_mut(key) {
                Some(entry) => {
                    if entry.0 + 1 != i {
                        let at = ctx.name_node(m).unwrap_or(m);
                        let message = if key.is_empty() {
                            "Move this constructor next to the other constructors".to_string()
                        } else {
                            format!(
                                "Move this overload of `{key}` next to the other `{key}` methods"
                            )
                        };
                        let previous = ctx.name_node(entry.1).unwrap_or(entry.1);
                        ctx.emit(at, message)
                            .secondary(previous, "previous one, separated by other members");
                    }
                    *entry = (i, m);
                }
                None => {
                    last.insert(key, (i, m));
                }
            }
        }
    }
}
