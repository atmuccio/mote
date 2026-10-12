//! `java/public-instance-field` — `public` mutable instance fields of classes.

use super::util::{binds_native_code, is_accessible_member, Mods};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["design", "encapsulation"],
    remediation_minutes: 10,
    description: r#"Detects `public` instance fields that are not `final`, in classes.

**Why it matters.** A public mutable field exposes the internal representation of a class:
any caller can change it without the class knowing, so invariants cannot be enforced, the
representation cannot change later without breaking callers, and there is no single place to
add validation, lazy computation, or change notification.

**How to fix.** Make the field `private` and add accessors as needed, make it `final` if it
never changes, or turn the class into a `record` if it is a plain data carrier.

```java
// Noncompliant
public class Range {
    public int low;
    public int high;
}

// Compliant
public record Range(int low, int high) { }
```

Not reported: annotated fields (frameworks such as JUnit's `@Rule` or command-line parsers
require public fields), fields of private, local, or anonymous classes, fields of enums and
records, and files that bind native code through JNA (which maps the public fields of a
`Structure` onto native memory)."#,
    ..RuleMeta::new(
        "java/public-instance-field",
        "java",
        "Mutable public instance field",
    )
};

pub struct PublicInstanceField;

impl<'t> JavaRule<'t> for PublicInstanceField {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["field_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let mods = Mods::of(node);
        if !mods.has("public") || mods.has("static") || mods.has("final") || mods.annotated() {
            return;
        }
        // Direct member of a named class.
        let mut up = ctx.ancestors(node);
        let (Some(body), Some(owner)) = (up.next(), up.next()) else {
            return;
        };
        if kind(body) != "class_body" || kind(owner) != "class_declaration" {
            return;
        }
        // JNA maps the public fields of a `Structure` onto native memory.
        if !is_accessible_member(node, ctx) || binds_native_code(ctx) {
            return;
        }
        for d in syntax::declarators(node) {
            if let Some(name) = d.field(Field::Name) {
                ctx.emit(
                    name,
                    format!(
                        "Make field `{}` private and provide accessors, or make it final",
                        ctx.text(name)
                    ),
                );
            }
        }
    }
}
