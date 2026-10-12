//! `java/compare-to-without-comparable` — `int compareTo(Self other)` in a class that does not
//! implement `Comparable`.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["api-misuse", "comparison"],
    remediation_minutes: 5,
    description: r#"Detects a method `int compareTo(Foo other)` declared in class `Foo` when the class
does not implement `Comparable` (it has no superclass and no interfaces).

**Why it matters.** The method looks like the natural ordering of the class, but nothing in
the JDK knows about it: `Collections.sort(list)` does not compile, and a `TreeSet<Foo>`, a
`TreeMap<Foo, …>` or `Arrays.sort(array)` compile fine and then throw `ClassCastException` at
run time, because they cast the elements to `Comparable`. Usually `implements Comparable<Foo>`
was simply forgotten.

**How to fix.** Declare `implements Comparable<Foo>` and mark the method `@Override`; or, if the
ordering is only one of several, turn it into a `Comparator<Foo>` constant.

```java
// Noncompliant
final class Version {
    public int compareTo(Version other) { … }
}

// Compliant
final class Version implements Comparable<Version> {
    @Override
    public int compareTo(Version other) { … }
}
```

Only classes and records without `extends` and `implements` clauses are checked (a supertype
declared elsewhere might be `Comparable`), and only when the parameter is the class itself.
Static methods and classes whose `compareTo` is used as a method reference in the same file
(`list.sort(Foo::compareTo)`, a deliberate comparator) are not reported."#,
    ..RuleMeta::new(
        "java/compare-to-without-comparable",
        "java",
        "compareTo() in a class that is not Comparable",
    )
};

pub struct CompareToWithoutComparable;

impl<'t> JavaRule<'t> for CompareToWithoutComparable {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_declaration"]
    }

    fn method_names(&self) -> &'static [&'static str] {
        &["compareTo"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) = node.field(Field::Name) else {
            return;
        };
        if ctx.text(name) != "compareTo" || syntax::has_modifier(node, "static") {
            return;
        }
        if node.field(Field::Type).is_none_or(|t| ctx.text(t) != "int") {
            return;
        }
        let mut params = syntax::parameters(node);
        let (Some(param), None) = (params.next(), params.next()) else {
            return;
        };
        let Some(pty) = param.field(Field::Type) else {
            return;
        };
        let Some(class) = ctx.parent(node).and_then(|b| ctx.parent(b)) else {
            return;
        };
        if !matches!(kind(class), "class_declaration" | "record_declaration") {
            return;
        }
        if class.field(Field::Superclass).is_some()
            || class.field(Field::Interfaces).is_some()
            || syntax::has_annotation(node, ctx.source(), "Override")
        {
            return;
        }
        let Some(class_name) = class.field(Field::Name).map(|n| ctx.text(n)) else {
            return;
        };
        let base = if kind(pty) == "generic_type" {
            syntax::first_named_child(pty).unwrap_or(pty)
        } else {
            pty
        };
        if ctx.text(base) != class_name {
            return;
        }
        // `Foo::compareTo` used as a comparator on purpose.
        if ctx.source().contains(&format!("{class_name}::compareTo")) {
            return;
        }
        ctx.emit(
            name,
            format!("Implement `Comparable<{class_name}>`; sorted collections and sort methods ignore this compareTo and fail with ClassCastException"),
        );
    }
}
