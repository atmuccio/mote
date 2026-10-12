//! `java/unused-private-method` — private methods that nothing in the file calls or references.

use super::util::{occurs_outside, Mods};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Medium,
    tags: &["dead-code", "unused"],
    remediation_minutes: 5,
    description: r##"Detects `private` methods that are never called or referenced from anywhere
else in the file.

**Why it matters.** A private method can only be used inside its own compilation unit. If
nothing there calls it, it is dead code: it has to be read, maintained and compiled for
nothing, and it often signals an unfinished refactoring or a call that was lost by mistake.

**How to fix.** Delete the method, or call it where it was meant to be called.

```java
// Noncompliant
class Report {
    public String render() { return header(); }
    private String header() { return "#"; }
    private String footer() { return "--"; }   // never called
}

// Compliant
class Report {
    public String render() { return header() + footer(); }
    private String header() { return "#"; }
    private String footer() { return "--"; }
}
```

Matching is by name and deliberately generous: the method counts as used when its name
appears as a whole word anywhere else in the file — a call on any receiver (overloads
included), a method reference (`this::name`, `Type::name`), a string literal (reflection, JUnit
`@MethodSource`), or even a comment. Occurrences inside the method itself (recursion) do not
count. Not reported: annotated methods (framework callbacks, `@SuppressWarnings("unused")`)
and the serialization hooks `writeObject`, `readObject`, `readObjectNoData`, `writeReplace`,
and `readResolve`."##,
    ..RuleMeta::new(
        "java/unused-private-method",
        "java",
        "Unused private method",
    )
};

const SERIALIZATION_HOOKS: &[&str] = &[
    "writeObject",
    "readObject",
    "readObjectNoData",
    "writeReplace",
    "readResolve",
];

/// File-level: private methods are collected during the walk; in `finish` each one is looked
/// up in the source text (candidates are rare, so most files cost nothing beyond the walk).
#[derive(Default)]
pub struct UnusedPrivateMethod<'t> {
    /// (name identifier, name, declaration byte range).
    candidates: Vec<(Node<'t>, &'t str, usize, usize)>,
}

impl<'t> JavaRule<'t> for UnusedPrivateMethod<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !Mods::of(node).has("private") || Mods::of(node).annotated() {
            return;
        }
        let Some(name_node) = ctx.name_node(node) else {
            return;
        };
        let name = ctx.text(name_node);
        if SERIALIZATION_HOOKS.contains(&name) {
            return;
        }
        self.candidates
            .push((name_node, name, node.start_byte(), node.end_byte()));
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        let src = ctx.source();
        for &(name_node, name, start, end) in &self.candidates {
            if !occurs_outside(src, name, start, end) {
                ctx.emit(
                    name_node,
                    format!("Remove the unused private method `{name}`"),
                );
            }
        }
    }
}
