//! `java/redundant-qualified-this` — `Outer.this.x` where plain `this` is the same object.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["clutter", "redundant"],
    remediation_minutes: 1,
    description: r#"Detects `Name.this` where `Name` is the class the code is directly in, so
`Name.this` and `this` are the same object.

**Why it matters.** A qualified `this` is how inner and anonymous classes reach their
enclosing instance; using it where no such distinction exists sends the reader looking for an
inner class that isn't there.

**How to fix.** Write `this` (or drop it where it isn't needed).

```java
// Noncompliant
class Counter {
    private int count;
    void increment() {
        Counter.this.count++;
    }
}

// Compliant
class Counter {
    private int count;
    void increment() {
        this.count++;
    }
}
```

Inside an inner, local, or anonymous class, `Outer.this` refers to a different object and is
never reported. Lambdas don't introduce a new `this`, so the qualification is redundant there
as well."#,
    ..RuleMeta::new(
        "java/redundant-qualified-this",
        "java",
        "Redundant qualified this",
    )
};

pub struct RedundantQualifiedThis;

impl<'t> JavaRule<'t> for RedundantQualifiedThis {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["field_access"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !ctx.text(node).ends_with("this") {
            return;
        }
        let Some(field) = util::fld(node, F.field) else {
            return;
        };
        if kind(field) != "this" {
            return;
        }
        let Some(obj) = util::fld(node, F.object) else {
            return;
        };
        if kind(obj) != "identifier" {
            return;
        }
        let Some(class) = ctx.enclosing_class(node) else {
            return;
        };
        if !syntax::is_type_declaration(class) {
            return;
        }
        let name = ctx.text(obj);
        if syntax::name_of(class, ctx.source()) != name {
            return;
        }
        ctx.emit(
            node,
            format!("Use `this` instead of `{name}.this`: both refer to the same object"),
        );
    }
}
