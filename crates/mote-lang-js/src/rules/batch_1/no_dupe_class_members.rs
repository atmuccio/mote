//! `js/no-dupe-class-members`

use crate::prelude::*;

pub struct NoDupeClassMembers;

pub static NO_DUPE_CLASS_MEMBERS: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["suspicious"],
    remediation_minutes: 5,
    description: r#"Flags class members (methods, fields, accessors) declared more than once with the same
name in the same class body.

**Why:** the later declaration silently replaces the earlier one, so one implementation is dead
code. This usually comes from a merge or copy-paste, and the surviving version may not be the one
the author expected.

**Fix:** remove or rename one of the members.

```js
class Cart {
  total() { return this.items.length; }
  total() { return sum(this.items); }   // flagged: replaces the first `total`

  get size() { return this._size; }
  set size(v) { this._size = v; }      // compliant: getter/setter pair
}
```

Static and instance members are separate namespaces. TypeScript overload signatures, abstract
members, and `declare` fields are not counted, and computed keys are compared only when they are
literals.
"#,
    ..RuleMeta::new(
        "js/no-dupe-class-members",
        "js",
        "Class members should not be declared twice",
    )
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Slot {
    Get,
    Set,
    Other,
}

impl JsRule for NoDupeClassMembers {
    fn meta(&self) -> &'static RuleMeta {
        &NO_DUPE_CLASS_MEMBERS
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::ClassBody]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::ClassBody(body) = node.kind() else {
            return;
        };
        if body.body.len() < 2 {
            return;
        }
        // (static, private, name, order, slot, key span)
        let mut members: Vec<(bool, bool, Cow<'a, str>, usize, Slot, Span)> = Vec::new();
        for (order, element) in body.body.iter().enumerate() {
            let (key, is_static, slot) = match element {
                ClassElement::MethodDefinition(m) => {
                    if m.r#type == MethodDefinitionType::TSAbstractMethodDefinition
                        || m.value.body.is_none()
                        || m.kind == MethodDefinitionKind::Constructor
                    {
                        continue;
                    }
                    let slot = match m.kind {
                        MethodDefinitionKind::Get => Slot::Get,
                        MethodDefinitionKind::Set => Slot::Set,
                        _ => Slot::Other,
                    };
                    (&m.key, m.r#static, slot)
                }
                ClassElement::PropertyDefinition(p) => {
                    if p.declare || p.r#type == PropertyDefinitionType::TSAbstractPropertyDefinition
                    {
                        continue;
                    }
                    (&p.key, p.r#static, Slot::Other)
                }
                ClassElement::AccessorProperty(p) => {
                    if p.r#type == AccessorPropertyType::TSAbstractAccessorProperty {
                        continue;
                    }
                    (&p.key, p.r#static, Slot::Other)
                }
                _ => continue,
            };
            let (private, name) = match key {
                PropertyKey::PrivateIdentifier(id) => (true, Cow::Borrowed(id.name.as_str())),
                _ => match helpers::property_key_name(key) {
                    Some(name) => (false, name),
                    None => continue,
                },
            };
            members.push((is_static, private, name, order, slot, key.span()));
        }
        if members.len() < 2 {
            return;
        }
        members.sort_by(|a, b| (a.0, a.1, &a.2, a.3).cmp(&(b.0, b.1, &b.2, b.3)));

        let mut group_start = 0;
        for i in 1..members.len() {
            let same = {
                let (a, b) = (&members[group_start], &members[i]);
                a.0 == b.0 && a.1 == b.1 && a.2 == b.2
            };
            if !same {
                group_start = i;
                continue;
            }
            let (_, private, ref name, _, slot, span) = members[i];
            // The first earlier member this one collides with (a getter and a setter pair up).
            let first = members[group_start..i]
                .iter()
                .find(|m| !matches!((m.4, slot), (Slot::Get, Slot::Set) | (Slot::Set, Slot::Get)))
                .map(|m| m.5);
            if let Some(first) = first {
                let shown: String = name.chars().take(40).collect();
                let hash = if private { "#" } else { "" };
                ctx.emit(
                    span,
                    format!("Remove or rename the duplicate class member '{hash}{shown}'"),
                )
                .secondary(first, "First declaration");
            }
        }
    }
}
