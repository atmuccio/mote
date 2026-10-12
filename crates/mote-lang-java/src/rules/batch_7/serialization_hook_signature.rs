//! `java/serialization-hook-signature` — serialization hooks (`writeObject`, `readObject`,
//! `readObjectNoData`, `readResolve`, `writeReplace`) declared with a signature that
//! serialization ignores.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["serialization"],
    remediation_minutes: 5,
    description: r#"Detects serialization hooks of a `Serializable` class whose declaration makes
Java serialization skip them:

* `writeObject(ObjectOutputStream)`, `readObject(ObjectInputStream)` and `readObjectNoData()`
  that are not `private`, are `static`, or don't return `void`;
* `readResolve()` and `writeReplace()` that are `static` or whose return type is not
  `Object`;
* `writeObject`, `readObject` and `readObjectNoData` declared in a record, where
  serialization never calls them.

**Why it matters.** Serialization finds these methods by reflection, by exact signature. A
hook with the wrong modifiers or return type is not an error — it is silently ignored. Custom
state is then neither written nor restored, validation in `readObject` never runs, and a
singleton's `readResolve` no longer protects it from duplication.

**How to fix.** Use the exact signatures:
`private void writeObject(ObjectOutputStream out) throws IOException`,
`private void readObject(ObjectInputStream in) throws IOException, ClassNotFoundException`,
`Object readResolve() throws ObjectStreamException` (any access).

```java
// Noncompliant
public void readObject(ObjectInputStream in) throws IOException, ClassNotFoundException { … }
private Config readResolve() { return INSTANCE; }

// Compliant
private void readObject(ObjectInputStream in) throws IOException, ClassNotFoundException { … }
private Object readResolve() { return INSTANCE; }
```

Only classes and records the resolver proves `Serializable` are checked; enums (whose
serialization ignores all these hooks) and `Externalizable` classes are skipped."#,
    ..RuleMeta::new(
        "java/serialization-hook-signature",
        "java",
        "Serialization hook with an ignored signature",
    )
};

pub struct SerializationHookSignature;

impl<'t> JavaRule<'t> for SerializationHookSignature {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_declaration"]
    }

    fn method_names(&self) -> &'static [&'static str] {
        &[
            "writeObject",
            "readObject",
            "readObjectNoData",
            "readResolve",
            "writeReplace",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name_node) = node.field(Field::Name) else {
            return;
        };
        let name = ctx.text(name_node);
        let stream = match name {
            "writeObject" => Some("ObjectOutputStream"),
            "readObject" => Some("ObjectInputStream"),
            "readObjectNoData" | "readResolve" | "writeReplace" => None,
            _ => return,
        };
        let mut params = syntax::parameters(node);
        let first = params.next();
        if params.next().is_some() || first.is_some() != stream.is_some() {
            return;
        }
        if let (Some(p), Some(stream)) = (first, stream) {
            let ty = p.field(Field::Type).map(|t| ctx.text(t)).unwrap_or("");
            if ty != stream && ty != format!("java.io.{stream}") {
                return;
            }
        }
        let Some(class) = ctx.enclosing_class(node) else {
            return;
        };
        if !matches!(kind(class), "class_declaration" | "record_declaration") {
            return;
        }
        if class
            .field(Field::Interfaces)
            .is_some_and(|i| ctx.text(i).contains("Externalizable"))
        {
            return;
        }
        let Some(t) = ctx.class_type(class) else {
            return;
        };
        if !ctx.is_subtype_of(&t, "java.io.Serializable") {
            return;
        }
        let ret = node.field(Field::Type).map(|t| ctx.text(t)).unwrap_or("");
        let is_static = syntax::has_modifier(node, "static");
        let problem = match name {
            "readResolve" | "writeReplace" => {
                if is_static {
                    "must not be static".to_string()
                } else if ret != "Object" && ret != "java.lang.Object" {
                    format!("must return `Object`, not `{ret}`")
                } else {
                    return;
                }
            }
            _ => {
                if kind(class) == "record_declaration" {
                    "is never called for records".to_string()
                } else if is_static {
                    "must not be static".to_string()
                } else if !syntax::has_modifier(node, "private") {
                    "must be private".to_string()
                } else if ret != "void" {
                    "must return void".to_string()
                } else {
                    return;
                }
            }
        };
        ctx.emit(
            name_node,
            format!("Fix this serialization hook: `{name}` {problem}, so serialization silently ignores it"),
        );
    }
}
