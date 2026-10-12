//! `java/final-field-could-be-static` — `private final int MAX = 10;` stored in every instance.

use super::util::{self, Mods, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["design", "performance"],
    remediation_minutes: 1,
    description: r#"Detects `final` instance fields of a primitive, boxed primitive or `String` type
initialized with a compile-time constant (`private final int retries = 3;`).

**Why it matters.** Every instance gets its own copy of a value that can never differ between
instances. Making the field `static` states that it is a class-wide constant, saves the
per-instance storage, and lets the compiler treat it as a constant everywhere.

**How to fix.** Add `static` (and, by convention, rename it in `UPPER_SNAKE` case).

```java
// Noncompliant
private final int maxRetries = 3;
private final String prefix = "job-";

// Compliant
private static final int MAX_RETRIES = 3;
private static final String PREFIX = "job-";
```

Only initializers made of literals and operators count (`60 * 1000`, `"a" + "b"`). Not
reported: annotated fields, fields of classes with Lombok annotations other than the logger ones
(`@Slf4j`, `@Log4j2`, … add a logger and nothing else) and of classes marked for persistence or
serialization frameworks (which read instance fields reflectively), and interface members
(already static). A non-static `serialVersionUID` is reported too: the
serialization mechanism ignores it unless it is `static`."#,
    ..RuleMeta::new(
        "java/final-field-could-be-static",
        "java",
        "Constant final field could be static",
    )
};

/// Class annotations of frameworks that serialize or persist instance fields.
const FIELD_READING_CLASS_ANNOTATIONS: &[&str] = &[
    "Entity",
    "Embeddable",
    "MappedSuperclass",
    "Document",
    "Table",
    "XmlRootElement",
    "XmlType",
    "XmlAccessorType",
    "JsonAutoDetect",
    "JsonSerialize",
    "JsonPropertyOrder",
    "JsonTypeInfo",
    "JsonInclude",
];

pub struct FinalFieldCouldBeStatic;

fn is_constant_type<'t>(ctx: &Ctx<'t>, ty: Node<'t>) -> bool {
    match kind(ty) {
        "integral_type" | "floating_point_type" | "boolean_type" => true,
        "type_identifier" | "scoped_type_identifier" => {
            let t = ctx.text(ty);
            let simple = t.strip_prefix("java.lang.").unwrap_or(t);
            matches!(
                simple,
                "String"
                    | "Integer"
                    | "Long"
                    | "Short"
                    | "Byte"
                    | "Character"
                    | "Boolean"
                    | "Float"
                    | "Double"
            ) && util::type_fqn(ctx, ty)
                .is_some_and(|f| f.strip_prefix("java.lang.") == Some(simple))
        }
        _ => false,
    }
}

impl<'t> JavaRule<'t> for FinalFieldCouldBeStatic {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["field_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let mods = Mods::of(node);
        if !mods.has("final") || mods.has("static") || mods.annotated() {
            return;
        }
        let Some(ty) = util::fld(node, F.ty) else {
            return;
        };
        if !is_constant_type(ctx, ty) {
            return;
        }
        let Some(class) = ctx.enclosing_class(node) else {
            return;
        };
        if !matches!(kind(class), "class_declaration" | "enum_declaration") {
            return;
        }
        let src = ctx.source();
        if util::lombok_annotations(ctx, class).any(|a| !util::LOMBOK_LOGGERS.contains(&a))
            || Mods::of(class)
                .annotations()
                .any(|a| FIELD_READING_CLASS_ANNOTATIONS.contains(&syntax::annotation_name(a, src)))
        {
            return;
        }
        for d in syntax::declarators(node) {
            if util::fld(d, F.dimensions).is_some() {
                continue;
            }
            let (Some(name), Some(value)) = (util::fld(d, F.name), util::fld(d, F.value)) else {
                continue;
            };
            if !util::is_literal_constant_expr(value, 0) {
                continue;
            }
            ctx.emit(
                name,
                format!(
                    "Make `{}` static: it holds the same constant in every instance",
                    ctx.text(name)
                ),
            );
        }
    }
}
