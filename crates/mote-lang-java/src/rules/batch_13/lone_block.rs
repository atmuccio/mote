//! `java/lone-block` — a nested `{ … }` block that declares nothing.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["confusing", "structure"],
    remediation_minutes: 2,
    description: r#"Detects a block statement `{ … }` nested directly in a method, constructor or
initializer body (or in another such block) that declares no local variable or class.

**Why it matters.** A bare block only limits the scope of what is declared inside it. When
nothing is declared it has no effect at all, and it reads like the body of a statement that
went missing — an `if`, a loop or an `else` deleted by an edit or a merge — so readers stop to
work out whether code was lost.

**How to fix.** Remove the braces, or restore the statement the block belongs to. To group
steps, extract a method with a descriptive name.

```java
// Noncompliant
void reset() {
    counter = 0;
    {
        cache.clear();
        listeners.clear();
    }
}

// Compliant
void reset() {
    counter = 0;
    cache.clear();
    listeners.clear();
}
```

Not reported: blocks that declare a local variable or a local class (limiting their scope is
the point), empty blocks (`java/empty-block`), labeled blocks (`label: { … break label; }`),
blocks that are a `case` group's body, and blocks of lambdas, `switch` rules, `synchronized`
and other statements."#,
    ..RuleMeta::new(
        "java/lone-block",
        "java",
        "Nested block without declarations",
    )
};

pub struct LoneBlock;

impl<'t> JavaRule<'t> for LoneBlock {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["block"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(parent) = ctx.parent(node) else {
            return;
        };
        if !matches!(kind(parent), "block" | "constructor_body") {
            return;
        }
        // A block directly in a class body is an initializer, whose parent is `class_body`, so
        // `parent` here is always a statement list.
        let mut stmts = util::statements(node).peekable();
        if stmts.peek().is_none() {
            return;
        }
        let declares = stmts.any(|s| {
            matches!(
                kind(s),
                "local_variable_declaration"
                    | "class_declaration"
                    | "record_declaration"
                    | "interface_declaration"
                    | "enum_declaration"
            )
        });
        if declares {
            return;
        }
        let open = node.child(0).unwrap_or(node);
        ctx.emit(
            open,
            "Remove this nested block or restore the statement it belongs to: it declares nothing",
        );
    }
}
