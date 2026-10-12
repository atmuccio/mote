//! `js/enum-initializers`

use crate::prelude::*;

pub struct EnumInitializers;

pub static ENUM_INITIALIZERS: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Incubating,
    tags: &["typescript", "enums", "pitfall"],
    remediation_minutes: 5,
    description: r#"Flags TypeScript enums that mix members with explicit values and members numbered implicitly
in a way that breaks when the enum changes: implicit members placed before the first explicit one
(`enum State { Idle, Busy, Done = 2 }`), and implicit members whose number coincides with an
explicit value (`enum Level { Low = 1, Medium, High = 2 }`, where `Medium` is also 2).

**Why:** an implicit member takes the number after the member before it. Implicit members in front
of explicit ones are numbered from 0 while the later values are fixed, so adding a member among
them shifts them silently and soon collides with the explicit values; two members with the same
number compare equal and one of them can no longer be told apart (in `switch`, in stored data,
in reverse lookups).

**Fix:** give every member an explicit value, or none at all.

```ts
enum State { Idle, Busy, Done = 2 }              // flagged: Idle and Busy are implicit
enum Level { Low = 1, Medium, High = 2 }         // flagged: Medium is 2 as well

enum State { Idle = 0, Busy = 1, Done = 2 }      // compliant
enum Direction { Up, Down, Left, Right }         // compliant: all implicit
enum Code { A = 100, B, C }                      // compliant: a chosen starting number
enum Order { Refresh = 10, Run, Debug, Collapse = 20, Clear } // compliant: numbered sections
```

Not reported: an explicit value followed by implicit members (a chosen starting number, or one
per section) as long as no implicit number collides with an explicit one. Collisions are only
computed for plain numeric values.
"#,
    ..RuleMeta::new(
        "js/enum-initializers",
        "js",
        "Enum members should not mix implicit and explicit values in a fragile way",
    )
};

impl JsRule for EnumInitializers {
    fn meta(&self) -> &'static RuleMeta {
        &ENUM_INITIALIZERS
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.is_typescript()
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::TSEnumDeclaration]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::TSEnumDeclaration(decl) = node.kind() else {
            return;
        };
        let members = &decl.body.members;
        let Some(first_explicit) = members.iter().position(|m| m.initializer.is_some()) else {
            return;
        };
        // Implicit members before the first explicit one.
        if first_explicit > 0 {
            let names: Vec<&str> = members[..first_explicit]
                .iter()
                .take(3)
                .map(|m| ctx.text(m.id.span()))
                .collect();
            let more = if first_explicit > 3 { ", …" } else { "" };
            ctx.emit(
                members[0].span,
                format!(
                    "Give '{}' explicit values like the later members of '{}': implicit members in front of explicit ones shift when the enum changes",
                    names.join("', '") + more,
                    decl.id.name
                ),
            );
            return;
        }
        // Implicit numbers that coincide with an explicit value.
        let explicit: Vec<f64> = members
            .iter()
            .filter_map(|m| m.initializer.as_ref().and_then(helpers::number_value))
            .collect();
        let mut next: Option<f64> = None;
        for member in members {
            match &member.initializer {
                Some(value) => next = helpers::number_value(value).map(|v| v + 1.0),
                None => {
                    let Some(value) = next else {
                        // After a computed or string value: unknown.
                        continue;
                    };
                    if explicit.contains(&value) {
                        ctx.emit(
                            member.span,
                            format!(
                                "Give '{}' an explicit value: it is numbered {value}, like another member of '{}'",
                                ctx.text(member.id.span()),
                                decl.id.name
                            ),
                        );
                        return;
                    }
                    next = Some(value + 1.0);
                }
            }
        }
    }
}
