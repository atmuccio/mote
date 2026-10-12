//! `java/deprecated-annotation-missing` — a `@deprecated` Javadoc tag without the
//! `@Deprecated` annotation (and, optionally, the reverse).

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["deprecation", "documentation"],
    params: &[ParamSpec {
        name: "require-javadoc-tag",
        default: ParamDefault::Bool(false),
        description: "Also report `@Deprecated` declarations whose Javadoc comment has no \
                      `@deprecated` tag explaining what to use instead.",
    }],
    remediation_minutes: 2,
    description: r#"Detects a declaration whose Javadoc has a `@deprecated` tag but which is not
annotated `@Deprecated`. With the `require-javadoc-tag` parameter, also reports the reverse:
a `@Deprecated` declaration with a Javadoc comment that does not say why or what replaces it.

**Why it matters.** The compiler, IDEs and tools only see the annotation: without
`@Deprecated`, callers get no deprecation warning, the element is not struck through, and
removal-tracking tools miss it — the Javadoc tag alone only reaches people who read the
generated documentation. Conversely, the annotation without the tag warns callers but gives
them no replacement.

**How to fix.** Add `@Deprecated` (with `since` / `forRemoval` if useful) next to the tag;
document the replacement in the `@deprecated` tag.

```java
// Noncompliant
/**
 * @deprecated use {@link #connect(Endpoint)} instead
 */
public void connect(String host) { ... }

// Compliant
/**
 * @deprecated use {@link #connect(Endpoint)} instead
 */
@Deprecated
public void connect(String host) { ... }
```

Only Javadoc comments placed immediately before the declaration are considered; a
`@deprecated` mentioned inside the text (not at the start of a line) is not a tag."#,
    ..RuleMeta::new(
        "java/deprecated-annotation-missing",
        "java",
        "Deprecation not marked with @Deprecated",
    )
};

#[derive(Default)]
pub struct DeprecatedAnnotationMissing {
    tag_gate: util::Gate,
    ann_gate: util::Gate,
}

/// Whether a Javadoc comment has a `@deprecated` block tag (at the start of a line).
fn has_deprecated_tag(javadoc: &str) -> bool {
    javadoc.lines().any(|line| {
        let l = line.trim_start();
        let l = l.strip_prefix("/**").unwrap_or(l);
        let l = l.trim_start_matches('*').trim_start();
        l.strip_prefix("@deprecated")
            .is_some_and(|rest| !rest.starts_with(|c: char| c.is_alphanumeric()))
    })
}

fn has_deprecated_annotation(ctx: &Ctx<'_>, decl: Node<'_>) -> bool {
    syntax::annotations(decl).any(|a| {
        matches!(
            syntax::annotation_name(a, ctx.source()),
            "Deprecated" | "java.lang.Deprecated"
        )
    })
}

impl<'t> JavaRule<'t> for DeprecatedAnnotationMissing {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "method_declaration",
            "constructor_declaration",
            "class_declaration",
            "interface_declaration",
            "enum_declaration",
            "record_declaration",
            "annotation_type_declaration",
            "field_declaration",
            "constant_declaration",
            "enum_constant",
            "annotation_type_element_declaration",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let tags = self.tag_gate.open(ctx, "@deprecated");
        let reverse =
            ctx.param_bool("require-javadoc-tag") && self.ann_gate.open(ctx, "@Deprecated");
        if !tags && !reverse {
            return;
        }
        let Some(prev) = ctx.prev_named_sibling(node) else {
            return;
        };
        if kind(prev) != "block_comment" {
            return;
        }
        let doc = ctx.text(prev);
        if !doc.starts_with("/**") {
            return;
        }
        let has_tag = tags && has_deprecated_tag(doc);
        if !has_tag && !reverse {
            return;
        }
        let annotated = has_deprecated_annotation(ctx, node);
        let at = report_node(node).unwrap_or(node);
        if has_tag && !annotated {
            ctx.emit(
                at,
                "Add `@Deprecated` here to match the `@deprecated` Javadoc tag: without the annotation callers get no deprecation warning",
            );
        } else if reverse && annotated && !has_deprecated_tag(doc) {
            ctx.emit(
                at,
                "Add a `@deprecated` tag to this Javadoc saying why the element is deprecated and what to use instead",
            );
        }
    }
}

/// The name of a declaration (the first declarator's name for fields).
fn report_node(decl: Node<'_>) -> Option<Node<'_>> {
    match kind(decl) {
        "field_declaration" | "constant_declaration" => syntax::declarators(decl)
            .next()
            .and_then(|d| d.field(Field::Name)),
        _ => decl.field(Field::Name),
    }
}
