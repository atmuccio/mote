//! `java/member-order` — fields, constructors and methods of a type out of the conventional
//! order (static fields, instance fields, constructors, methods).

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["conventions", "structure"],
    remediation_minutes: 2,
    description: r#"Detects members of a class, enum, record or interface declared out of the
conventional order: static fields first, then instance fields, then constructors, then methods.

**Why it matters.** When every type is laid out the same way, a reader knows where to look for
its state and how it is created without scanning the whole file. A field declared between two
methods near the bottom of a class is state that most readers will not notice, and a
constructor among the methods hides how instances come to exist.

**How to fix.** Move the member up to its group.

```java
// Noncompliant
class Account {
    private final String id;
    Account(String id) { this.id = id; }
    String id() { return id; }
    private static final int MAX_NAME = 64;   // a static field after the methods
}

// Compliant
class Account {
    private static final int MAX_NAME = 64;
    private final String id;
    Account(String id) { this.id = id; }
    String id() { return id; }
}
```

Each member that comes after a member of a later group is reported, with that member as
secondary location. Nested types, initializer blocks and enum constants are not ordered by this
rule, and anonymous classes are not checked. Visibility order within a group is not checked.
The rule is off by default and reports a lot on code that groups members by feature instead:
it is part of the `conventions` profile."#,
    ..RuleMeta::new(
        "java/member-order",
        "java",
        "Members out of the conventional order",
    )
};

pub struct MemberOrder;

const GROUPS: [&str; 4] = [
    "the static fields",
    "the instance fields",
    "the constructors",
    "the methods",
];

/// The group of a member: 0 static fields, 1 instance fields, 2 constructors, 3 methods.
fn group(member: Node<'_>, in_interface: bool) -> Option<usize> {
    match kind(member) {
        "constant_declaration" => Some(0),
        "field_declaration" => Some(if in_interface || syntax::has_modifier(member, "static") {
            0
        } else {
            1
        }),
        "constructor_declaration" | "compact_constructor_declaration" => Some(2),
        "method_declaration" => Some(3),
        _ => None,
    }
}

/// The name of a member (the first variable of a field declaration), or the member itself.
fn member_name<'t>(ctx: &Ctx<'t>, member: Node<'t>) -> Node<'t> {
    let name = match kind(member) {
        "field_declaration" | "constant_declaration" => syntax::declarators(member)
            .next()
            .and_then(|d| d.field(Field::Name)),
        _ => ctx.name_node(member),
    };
    name.unwrap_or(member)
}

impl<'t> JavaRule<'t> for MemberOrder {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["class_body", "interface_body", "enum_body"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        // Named types only (not anonymous classes or enum constant bodies).
        if !ctx.parent(node).is_some_and(syntax::is_type_declaration) {
            return;
        }
        let in_interface = kind(node) == "interface_body";
        // The latest group seen and the first member that opened it.
        let mut max: Option<(usize, Node<'t>)> = None;
        for m in ctx.members(node) {
            let Some(g) = group(m, in_interface) else {
                continue;
            };
            match max {
                Some((top, opener)) if g < top => {
                    let at = member_name(ctx, m);
                    let what = match g {
                        0 => "static field",
                        1 => "field",
                        _ => "constructor",
                    };
                    ctx.emit(
                        at,
                        format!("Move the {what} `{}` above {}", ctx.text(at), GROUPS[top]),
                    )
                    .secondary(
                        member_name(ctx, opener),
                        "first of the group that should come after it",
                    );
                }
                Some((top, _)) if g == top => {}
                _ => max = Some((g, m)),
            }
        }
    }
}
