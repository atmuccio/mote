//! `java/unused-private-class` — private nested types that nothing outside them references.

use super::util::{occurs_outside, Mods};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["dead-code", "unused"],
    remediation_minutes: 5,
    description: r#"Detects `private` nested classes, interfaces, enums, records, and annotation
types that are not referenced anywhere outside their own declaration.

**Why it matters.** A private nested type is only visible inside its top-level class. If
nothing there mentions it, it is dead code that readers still have to understand and that
keeps its own dependencies alive.

**How to fix.** Delete the type, or use it where it was meant to be used.

```java
// Noncompliant
public class Parser {
    private static class Token { }          // never used
    public Node parse(String s) { … }
}

// Compliant
public class Parser {
    public Node parse(String s) { … }
}
```

Uses are matched by simple name and deliberately generously: the type counts as used when its
name appears as a whole word anywhere else in the file — as a type, in a qualified name, in a
string literal (reflection such as `"Outer$Inner"`), or even in a comment. References from
inside the type itself (its constructors, factory methods) do not count. Annotated types are
not reported."#,
    ..RuleMeta::new(
        "java/unused-private-class",
        "java",
        "Unused private nested type",
    )
};

/// File-level: private nested types are collected during the walk; in `finish` each one is
/// looked up in the source text (candidates are rare, so most files cost nothing more).
#[derive(Default)]
pub struct UnusedPrivateClass<'t> {
    /// (name identifier, name, declaration byte range).
    candidates: Vec<(Node<'t>, &'t str, usize, usize)>,
}

impl<'t> JavaRule<'t> for UnusedPrivateClass<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "class_declaration",
            "interface_declaration",
            "enum_declaration",
            "record_declaration",
            "annotation_type_declaration",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !Mods::of(node).has("private") || Mods::of(node).annotated() {
            return;
        }
        let Some(name_node) = node.field(Field::Name) else {
            return;
        };
        self.candidates.push((
            name_node,
            ctx.text(name_node),
            node.start_byte(),
            node.end_byte(),
        ));
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        let src = ctx.source();
        for &(name_node, name, start, end) in &self.candidates {
            if !occurs_outside(src, name, start, end) {
                ctx.emit(
                    name_node,
                    format!("Remove the unused private type `{name}`"),
                );
            }
        }
    }
}
