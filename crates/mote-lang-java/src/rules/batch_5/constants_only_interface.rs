//! `java/constants-only-interface` — an interface used only as a bag of constants.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["design"],
    remediation_minutes: 10,
    description: r#"Detects interfaces that declare nothing but constants.

**Why it matters.** An interface defines a type. Using one as a namespace for constants
invites classes to "implement" it just to use the names unqualified, which leaks the
constants into the public API of those classes and all their subclasses — forever, since
removing an interface is a breaking change.

**How to fix.** Move the constants to a `final` class with a private constructor (or to an
`enum` when they form a closed set), and use `import static` where short names are wanted.

```java
// Noncompliant
public interface Limits {
    int MAX_USERS = 100;
    int MAX_ROLES = 10;
}

// Compliant
public final class Limits {
    public static final int MAX_USERS = 100;
    public static final int MAX_ROLES = 10;

    private Limits() { }
}
```

Interfaces that extend other interfaces, or that declare methods or nested types, are not
reported."#,
    ..RuleMeta::new(
        "java/constants-only-interface",
        "java",
        "Interface holding only constants",
    )
};

pub struct ConstantsOnlyInterface;

impl<'t> JavaRule<'t> for ConstantsOnlyInterface {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["interface_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if named_children(node).any(|c| kind(c) == "extends_interfaces") {
            return;
        }
        let mut constants = 0;
        for m in util::class_members(ctx, node) {
            if kind(m) != "constant_declaration" {
                return;
            }
            constants += 1;
        }
        if constants == 0 {
            return;
        }
        let Some(name) = util::fld(node, F.name) else {
            return;
        };
        ctx.emit(
            name,
            format!(
                "Move the constants of interface `{}` to a final class or an enum",
                ctx.text(name)
            ),
        );
    }
}
