//! `java/untyped-to-array-cast` — `(String[]) list.toArray()` always throws.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["collections", "api-misuse"],
    remediation_minutes: 2,
    description: r#"Detects the result of a no-argument `toArray()` on a collection or stream cast
to a more specific array type: `(String[]) names.toArray()`.

**Why it matters.** `Collection.toArray()` and `Stream.toArray()` create an array whose runtime
type is `Object[]`, whatever the element type. An `Object[]` is never a `String[]`, so the
cast throws `ClassCastException` every time it runs (unless the collection is empty and the
code path is never taken in tests).

**How to fix.** Ask for a typed array.

```java
// Noncompliant
String[] names = (String[]) nameList.toArray();

// Compliant
String[] names = nameList.toArray(new String[0]);
String[] same = nameList.stream().toArray(String[]::new);
```

Only receivers the resolver proves to be a `Collection` or a `Stream` are reported; casts to
`Object[]` and to arrays of a type variable (`(T[])`, whose erasure is `Object[]`) are not."#,
    ..RuleMeta::new(
        "java/untyped-to-array-cast",
        "java",
        "Untyped toArray() result cast to a specific array type",
    )
};

pub struct UntypedToArrayCast;

impl<'t> JavaRule<'t> for UntypedToArrayCast {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["cast_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let (Some(ty), Some(value)) = (node.field(Field::Type), node.field(Field::Value)) else {
            return;
        };
        if kind(ty) != "array_type" {
            return;
        }
        let call = syntax::strip_parens(value);
        if ctx.call_name(call) != Some("toArray") || syntax::argument_count(call) != 0 {
            return;
        }
        let Some(recv) = call.field(Field::Object) else {
            return;
        };
        let Some(elem) = syntax::array_element(ty) else {
            return;
        };
        match kind(elem) {
            "type_identifier" | "scoped_type_identifier" | "generic_type" => {}
            _ => return,
        }
        let Some(JType::Class(ct)) = ctx.resolve_type(elem) else {
            return;
        };
        if ct.name == "java.lang.Object" {
            return;
        }
        let Some(rt) = ctx.type_of(recv) else {
            return;
        };
        if !ctx.is_subtype_of(&rt, "java.util.Collection")
            && !ctx.is_subtype_of(&rt, "java.util.stream.Stream")
        {
            return;
        }
        ctx.emit(
            node,
            format!(
                "Use `toArray(new {}[0])`: `toArray()` returns an `Object[]`, so this cast always throws ClassCastException",
                ctx.text(elem)
            ),
        );
    }
}
