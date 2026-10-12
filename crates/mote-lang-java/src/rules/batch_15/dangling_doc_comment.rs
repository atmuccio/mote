//! `java/dangling-doc-comment` — a `/** … */` comment that documents nothing: after the
//! annotations of a declaration, inside a method body, or followed by no declaration.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Incubating,
    default_enabled: false,
    tags: &["conventions", "documentation"],
    remediation_minutes: 1,
    description: r#"Detects documentation comments (`/** … */`) in places where the
documentation tool does not attach them to anything:

* between a declaration's annotations and the rest of the declaration;
* inside a method, constructor or initializer body, before a statement or a local variable;
* in a type body before an initializer block, or at the end of the body with no member after
  it.

**Why it matters.** A doc comment is only read when it directly precedes a declaration. In the
places above it is silently dropped from the generated documentation and from IDE hovers, so
the author believes something is documented when it is not. Inside a method body the `/**`
form also suggests an API comment where an ordinary note was meant.

**How to fix.** Move the comment before the declaration (above its annotations), or turn it
into a plain `/* … */` or `//` comment when it is a note for readers of the code.

```java
// Noncompliant
@Override
/** Returns the cached size. */
public int size() { … }

void run() {
    /** Retry three times. */
    int attempts = 3;
}

// Compliant
/** Returns the cached size. */
@Override
public int size() { … }

void run() {
    // Retry three times.
    int attempts = 3;
}
```

Not reported: comments before a declaration of any kind (local classes and records included),
comments before the `package` or `import` lines (file headers and package documentation),
banner comments that start with three or more stars (`/*****`), and `/**` comments inside
expressions. The rule is off by default: it is part of the `conventions` profile."#,
    ..RuleMeta::new(
        "java/dangling-doc-comment",
        "java",
        "Doc comment that documents nothing",
    )
};

pub struct DanglingDocComment;

const AFTER_ANNOTATIONS: &str =
    "Move this doc comment above the annotations: after them it documents nothing";

/// Kinds a doc comment documents when it directly precedes them.
fn is_documentable(node: Node<'_>) -> bool {
    syntax::is_type_declaration(node)
        || matches!(
            kind(node),
            "method_declaration"
                | "constructor_declaration"
                | "compact_constructor_declaration"
                | "field_declaration"
                | "constant_declaration"
                | "annotation_type_element_declaration"
                | "enum_constant"
                | "enum_body_declarations"
                | "module_declaration"
                | "package_declaration"
                | "import_declaration"
        )
}

impl<'t> JavaRule<'t> for DanglingDocComment {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["block_comment"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !util::is_doc_comment(ctx.text(node)) {
            return;
        }
        let Some(parent) = ctx.parent(node) else {
            return;
        };
        let in_body = match kind(parent) {
            "modifiers" => {
                // Between annotations and keyword modifiers.
                let after_annotation = syntax::children(parent)
                    .take_while(|c| c.id() != node.id())
                    .any(|c| matches!(kind(c), "annotation" | "marker_annotation"));
                if after_annotation {
                    ctx.emit(node, AFTER_ANNOTATIONS);
                }
                return;
            }
            "block" | "constructor_body" | "switch_block_statement_group" => true,
            "class_body"
            | "interface_body"
            | "enum_body"
            | "enum_body_declarations"
            | "annotation_type_body"
            | "program" => false,
            _ if is_documentable(parent) => {
                // Right after the modifiers of a declaration that has no keyword modifier
                // (`@Override /** … */ String name()`).
                let previous = syntax::children(parent)
                    .take_while(|c| c.id() != node.id())
                    .filter(|c| c.is_named() && !syntax::is_comment(*c))
                    .last();
                let after_annotation = previous.is_some_and(|p| {
                    kind(p) == "modifiers"
                        && syntax::named_children(p)
                            .any(|c| matches!(kind(c), "annotation" | "marker_annotation"))
                });
                if after_annotation {
                    ctx.emit(node, AFTER_ANNOTATIONS);
                }
                return;
            }
            _ => return,
        };
        match util::next_code(parent, node) {
            Some(next) if is_documentable(next) => {}
            Some(next) if kind(next) == "ERROR" => {}
            Some(_) if in_body => {
                ctx.emit(
                    node,
                    "Use a plain comment here: a doc comment inside a body documents nothing",
                );
            }
            _ => {
                ctx.emit(
                    node,
                    "Use a plain comment here, or move it before the declaration it describes: \
                     no declaration follows it",
                );
            }
        }
    }
}
