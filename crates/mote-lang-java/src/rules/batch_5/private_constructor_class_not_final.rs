//! `java/private-constructor-class-not-final` — a class whose constructors are all private
//! but which is not declared `final`.

use super::util::{self, Mods, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["design"],
    remediation_minutes: 1,
    description: r#"Detects classes whose constructors are all `private` but that are not
declared `final`.

**Why it matters.** With only private constructors, no class outside this file can extend it
anyway. Saying so with `final` documents the intent (a utility class, a singleton, a class
built through a factory method), lets tools and readers rely on it, and turns an accidental
nested subclass into a compile error.

**How to fix.** Add `final`.

```java
// Noncompliant
public class Strings {
    private Strings() { }
    public static boolean isBlank(String s) { ... }
}

// Compliant
public final class Strings {
    private Strings() { }
    public static boolean isBlank(String s) { ... }
}
```

Not reported: classes without any explicit constructor, `abstract`, `sealed`, and
`non-sealed` classes, classes with Lombok annotations that may generate constructors or subclasses
(`@Data`, `@Value`, `@Builder`, `@AllArgsConstructor`, … — accessor, `toString` and logger
annotations such as `@Getter` or `@Slf4j` are fine), and classes extended in the same file (by a
nested, local, or anonymous class)."#,
    ..RuleMeta::new(
        "java/private-constructor-class-not-final",
        "java",
        "Class with only private constructors is not final",
    )
};

/// Lombok class annotations that generate neither constructors nor subclasses.
const LOMBOK_WITHOUT_CONSTRUCTORS: &[&str] = &[
    "Getter",
    "Setter",
    "ToString",
    "EqualsAndHashCode",
    "Accessors",
    "FieldNameConstants",
    "FieldDefaults",
];

#[derive(Default)]
pub struct PrivateConstructorClassNotFinal<'t> {
    candidates: Vec<(Node<'t>, &'t str)>,
    /// Simple names of every class extended in this file.
    extended: Vec<&'t str>,
}

fn simple_name<'t>(ctx: &Ctx<'t>, ty: Node<'t>) -> &'t str {
    let t = ctx.text(util::type_base(ty));
    t.rsplit('.').next().unwrap_or(t).trim()
}

impl<'t> JavaRule<'t> for PrivateConstructorClassNotFinal<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["class_declaration", "object_creation_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if kind(node) == "object_creation_expression" {
            // Only anonymous classes (`new T() { … }`) end with a brace.
            if !ctx.text(node).ends_with('}') {
                return;
            }
            if util::anonymous_body(node).is_some() {
                if let Some(ty) = util::fld(node, F.ty) {
                    self.extended.push(simple_name(ctx, ty));
                }
            }
            return;
        }
        if let Some(sup) = util::fld(node, F.superclass) {
            if let Some(ty) = syntax::first_named_child(sup) {
                self.extended.push(simple_name(ctx, ty));
            }
        }
        let mods = Mods::of(node);
        if mods.has("final") || mods.has("abstract") || mods.has("sealed") || mods.has("non-sealed")
        {
            return;
        }
        let mut constructors = 0;
        for m in util::class_members(ctx, node) {
            if kind(m) == "constructor_declaration" {
                if !Mods::of(m).has("private") {
                    return;
                }
                constructors += 1;
            }
        }
        if constructors == 0
            || util::lombok_annotations(ctx, node).any(|a| {
                !util::LOMBOK_LOGGERS.contains(&a) && !LOMBOK_WITHOUT_CONSTRUCTORS.contains(&a)
            })
        {
            return;
        }
        if let Some(name) = util::fld(node, F.name) {
            self.candidates.push((name, ctx.text(name)));
        }
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        for &(node, name) in &self.candidates {
            if self.extended.contains(&name) {
                continue;
            }
            ctx.emit(
                node,
                format!("Make class `{name}` final: all its constructors are private"),
            );
        }
    }
}
