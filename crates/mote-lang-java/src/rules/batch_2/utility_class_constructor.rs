//! `java/utility-class-constructor` — static-only classes should hide their constructor.

use super::util::Mods;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["design"],
    remediation_minutes: 2,
    description: r#"Detects utility classes — classes whose members are all `static`, with at
least one static method — that declare no constructor and therefore get an implicit public
one.

**Why it matters.** Instantiating a utility class makes no sense, yet the implicit public
constructor allows `new StringHelper()` and suggests to readers (and to code completion) that
instances are meaningful. It also allows subclassing, which cannot add anything useful.

**How to fix.** Add a `private` constructor (optionally throwing), and consider making the
class `final`.

```java
// Noncompliant
public class Strings {
    public static boolean isBlank(String s) { return s == null || s.isBlank(); }
}

// Compliant
public final class Strings {
    private Strings() { }
    public static boolean isBlank(String s) { return s == null || s.isBlank(); }
}
```

Not reported: classes that declare any constructor, abstract classes, classes that extend
another class or implement interfaces, annotated classes (frameworks may instantiate them;
Lombok's `@UtilityClass` generates the constructor), classes declaring `main`, `premain` or
`agentmain` (application and agent entry points), classes that this file instantiates
(`new Model()`, `Model::new`: whatever their members, they are used as objects), private
nested classes, and classes with instance members or non-static nested classes. Annotations that say nothing about instances do not exempt a class:
`@Deprecated`, `@SuppressWarnings`, API-status and tooling markers (`@API`, `@Beta`,
`@IgnoreJRERequirement`, `@SuppressFBWarnings`), nullness defaults, and Lombok's logger
annotations (`@Slf4j`, `@Log4j2`, …)."#,
    ..RuleMeta::new(
        "java/utility-class-constructor",
        "java",
        "Utility class without a private constructor",
    )
};

pub struct UtilityClassConstructor;

/// Class annotations that neither instantiate the class nor generate a constructor.
const HARMLESS_ANNOTATIONS: &[&str] = &[
    "Deprecated",
    "SuppressWarnings",
    "SuppressFBWarnings",
    "API",
    "Beta",
    "IgnoreJRERequirement",
    "ThreadSafe",
    "Immutable",
    "ParametersAreNonnullByDefault",
    "NonNullByDefault",
    "NullMarked",
];

/// Lombok's logger annotations (a static logger field, nothing else).
const LOMBOK_LOGGERS: &[&str] = &[
    "Log",
    "Slf4j",
    "XSlf4j",
    "Log4j",
    "Log4j2",
    "CommonsLog",
    "JBossLog",
    "Flogger",
    "CustomLog",
];

/// Whether some annotation of the class may mean that a framework instantiates it or that a
/// constructor is generated: anything but the harmless markers and Lombok's loggers.
fn has_meaningful_annotation<'t>(ctx: &Ctx<'t>, class: Node<'t>) -> bool {
    let imports_lombok = ctx
        .imports()
        .iter()
        .any(|i| i.path == "lombok" || i.path.starts_with("lombok."));
    syntax::annotations(class).any(|a| {
        let full = a.field(Field::Name).map_or("", |n| ctx.text(n));
        let simple = full.rsplit('.').next().unwrap_or(full);
        if HARMLESS_ANNOTATIONS.contains(&simple) {
            return false;
        }
        let lombok = full.starts_with("lombok.") || (imports_lombok && !full.contains('.'));
        !(lombok && LOMBOK_LOGGERS.contains(&simple))
    })
}

/// Whether the file creates an instance of the class `name`: `new Name(`, `new Outer.Name<>(`
/// or `Name::new`.
fn instantiated_in_file(src: &str, name: &str) -> bool {
    let b = src.as_bytes();
    let is_word = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
    src.match_indices(name).any(|(i, _)| {
        let end = i + name.len();
        if (i > 0 && is_word(b[i - 1])) || b.get(end).is_some_and(|c| is_word(*c)) {
            return false;
        }
        let after = src[end..].trim_start();
        if after.starts_with("::") && after[2..].trim_start().starts_with("new") {
            return true;
        }
        if !(after.starts_with('(') || after.starts_with('<')) {
            return false;
        }
        // Skip a qualification (`Outer.`) before the name, then expect the `new` keyword.
        let mut before = src[..i].trim_end();
        while let Some(rest) = before.strip_suffix('.') {
            before = rest
                .trim_end()
                .trim_end_matches(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '$')
                .trim_end();
        }
        before.ends_with("new")
            && !before.as_bytes()[..before.len() - 3]
                .last()
                .is_some_and(|c| is_word(*c))
    })
}

impl<'t> JavaRule<'t> for UtilityClassConstructor {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["class_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if node.field(Field::Superclass).is_some()
            || node.field(Field::Interfaces).is_some()
            || Mods::of(node).has("abstract")
            || Mods::of(node).has("private")
        {
            return;
        }
        // Top-level or a member of another type (not a local class).
        // Top-level, a static member class, or a class nested in an interface (implicitly static).
        match ctx.parent(node).map(kind) {
            Some("program" | "interface_body" | "annotation_type_body") => {}
            Some("class_body" | "enum_body_declarations") if Mods::of(node).has("static") => {}
            _ => return,
        }
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        let mut static_methods = 0usize;
        for m in ctx.members(body) {
            match kind(m) {
                "method_declaration" => {
                    if !Mods::of(m).has("static")
                        || matches!(
                            syntax::name_of(m, ctx.source()),
                            "main" | "premain" | "agentmain"
                        )
                    {
                        return;
                    }
                    static_methods += 1;
                }
                "field_declaration" => {
                    if !Mods::of(m).has("static") {
                        return;
                    }
                }
                "class_declaration" => {
                    if !Mods::of(m).has("static") {
                        return;
                    }
                }
                "interface_declaration"
                | "enum_declaration"
                | "record_declaration"
                | "annotation_type_declaration"
                | "static_initializer" => {}
                // Constructors, instance initializers, anything unexpected.
                _ => return,
            }
        }
        if static_methods == 0
            || (Mods::of(node).annotated() && has_meaningful_annotation(ctx, node))
        {
            return;
        }
        let Some(name) = node.field(Field::Name) else {
            return;
        };
        if instantiated_in_file(ctx.source(), ctx.text(name)) {
            return;
        }
        ctx.emit(
            name,
            format!(
                "Add a private constructor to utility class `{}` to prevent instantiation",
                ctx.text(name)
            ),
        );
    }
}
