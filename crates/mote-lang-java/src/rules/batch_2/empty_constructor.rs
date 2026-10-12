//! `java/empty-constructor` — non-private constructors with an empty body and no explanation.

use super::util::Mods;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    default_enabled: false,
    tags: &["unused", "documentation"],
    remediation_minutes: 2,
    description: r#"Detects constructors whose body is empty and contains no comment.

**Why it matters.** An empty constructor is either redundant — when it is the only one and
takes no parameters, the compiler generates the same default constructor — or it exists for a
reason the code does not state: a framework or serializer that needs a no-argument
constructor next to the real ones, or an API that must stay source-compatible. Without a
comment, readers cannot tell which, and a later cleanup may remove one that a framework
needs, or keep one nobody uses.

**How to fix.** Remove a redundant constructor, or add a comment inside the body saying why
it is there.

```java
// Noncompliant
public class Invoice {
    public Invoice() {}
    public Invoice(String number) { this.number = number; }
}

// Compliant
public class Invoice {
    public Invoice() {
        // Required by the JSON mapper.
    }
    public Invoice(String number) { this.number = number; }
}
```

Off by default: many code bases keep empty no-argument constructors for frameworks as a
matter of course. Not reported: `private` constructors (the usual way to prevent
instantiation of utility classes and singletons), enum constructors, annotated constructors
(`@Inject`, `@JsonCreator`, …, whose purpose the annotation states), constructors that call
`super(…)` or `this(…)`, bodies followed by a comment on the same line
(`Money() {} // for JPA`), and test files."#,
    ..RuleMeta::new(
        "java/empty-constructor",
        "java",
        "Empty constructor without explanation",
    )
};

pub struct EmptyConstructor;

impl<'t> JavaRule<'t> for EmptyConstructor {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["constructor_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if ctx.is_test() {
            return;
        }
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        // No statements (an explicit `super(…)` / `this(…)` counts as one) and no comment.
        if body.named_child_count() != 0 || body.has_error() {
            return;
        }
        let mods = Mods::of(node);
        if mods.has("private") || mods.annotated() {
            return;
        }
        let Some(parent) = ctx.parent(node) else {
            return;
        };
        if kind(parent) != "class_body" {
            // Enum constructors are implicitly private.
            return;
        }
        // A trailing comment on the line of the closing brace explains the empty body.
        if let Some(next) = ctx.next_sibling(node) {
            if syntax::is_comment(next) && next.start_position().row == body.end_position().row {
                return;
            }
        }
        let Some(name) = node.field(Field::Name) else {
            return;
        };
        let only_ctor = ctx
            .members(parent)
            .filter(|m| kind(*m) == "constructor_declaration")
            .nth(1)
            .is_none();
        let no_params = syntax::parameters(node).next().is_none()
            && node.field(Field::Parameters).is_some_and(|p| {
                syntax::named_children(p).all(|c| kind(c) != "receiver_parameter")
            });
        let throws = syntax::named_children(node).any(|c| kind(c) == "throws");
        let msg = if only_ctor && no_params && !throws {
            format!(
                "Remove this empty constructor `{}` (the compiler generates the same default constructor) or add a comment explaining it",
                ctx.text(name)
            )
        } else {
            format!(
                "Add a comment explaining why constructor `{}` is empty",
                ctx.text(name)
            )
        };
        ctx.emit(name, msg);
    }
}
