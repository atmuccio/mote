//! `java/unretained-annotation-lookup` — reflection asked for an annotation that is not kept at
//! run time (`isAnnotationPresent(Override.class)`).

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["reflection", "annotations"],
    remediation_minutes: 5,
    description: r#"Detects reflective annotation lookups — `isAnnotationPresent`,
`getAnnotation`, `getDeclaredAnnotation`, `getAnnotationsByType`,
`getDeclaredAnnotationsByType` — for an annotation that is not retained at run time:
`Override` and `SuppressWarnings` (source-only), or an annotation type declared in the same
file without `@Retention(RetentionPolicy.RUNTIME)`.

**Why it matters.** An annotation's retention decides whether it exists at run time. Source
annotations are discarded by the compiler, and the default retention (`CLASS`) keeps them in
the class file but hides them from reflection. The lookup therefore always answers `false` /
`null`, so the feature built on it — a plug-in scan, a validation, a marker — silently never
triggers.

**How to fix.** Add `@Retention(RetentionPolicy.RUNTIME)` to your annotation type; for
source-only JDK annotations, find another way to express the information.

```java
// Noncompliant
@interface Audited { }
boolean audited = method.isAnnotationPresent(Audited.class);

// Compliant
@Retention(RetentionPolicy.RUNTIME)
@interface Audited { }
```

Annotation types declared in other files are unknown and not reported."#,
    ..RuleMeta::new(
        "java/unretained-annotation-lookup",
        "java",
        "Reflective lookup of an annotation that is not retained",
    )
};

#[derive(Default)]
pub struct UnretainedAnnotationLookup<'t> {
    /// (lookup call, class literal type node).
    lookups: Vec<(Node<'t>, Node<'t>)>,
    /// Annotation types of this file: (simple name, retained at run time).
    declared: Vec<(&'t str, bool)>,
}

const LOOKUPS: &[&str] = &[
    "isAnnotationPresent",
    "getAnnotation",
    "getDeclaredAnnotation",
    "getAnnotationsByType",
    "getDeclaredAnnotationsByType",
];

impl<'t> JavaRule<'t> for UnretainedAnnotationLookup<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation", "annotation_type_declaration"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        LOOKUPS
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if kind(node) == "annotation_type_declaration" {
            let name = syntax::name_of(node, ctx.source());
            let runtime = syntax::annotations(node).any(|a| {
                util::last_segment(syntax::annotation_name(a, ctx.source())) == "Retention"
                    && ctx.text(a).contains("RUNTIME")
            });
            self.declared.push((name, runtime));
            return;
        }
        let Some(name) = ctx.call_name(node) else {
            return;
        };
        if !LOOKUPS.contains(&name) || node.field(Field::Object).is_none() {
            return;
        }
        let mut args = util::args(node);
        let (Some(arg), None) = (args.next(), args.next()) else {
            return;
        };
        let lit = syntax::strip_parens(arg);
        if kind(lit) != "class_literal" {
            return;
        }
        if let Some(ty) = syntax::first_named_child(lit) {
            self.lookups.push((node, ty));
        }
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        for &(call, ty) in &self.lookups {
            let text = ctx.text(ty);
            let simple = util::last_segment(text);
            let why = if matches!(simple, "Override" | "SuppressWarnings") {
                let fqn = format!("java.lang.{simple}");
                if ctx.resolve_type_name(text, ty).as_deref() != Some(fqn.as_str()) {
                    continue;
                }
                "is a source-only annotation"
            } else {
                let mut same = self.declared.iter().filter(|(n, _)| *n == simple);
                match (same.next(), same.next()) {
                    (Some((_, false)), None) => "is not declared with @Retention(RUNTIME)",
                    _ => continue,
                }
            };
            ctx.emit(
                ty,
                format!("This lookup always fails: `{simple}` {why}, so reflection never sees it"),
            )
            .secondary(call, "reflective lookup");
        }
    }
}
