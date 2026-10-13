//! `java/dangling-doc-comment` — a `/** … */` comment (or a `///` run, Java 23) that documents
//! nothing: after the annotations of a declaration, inside a method body, or followed by no
//! declaration.

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
expressions.

Since Java 23 a run of `///` lines is a documentation comment too (Markdown syntax). Such a
run is reported after annotations and in a type body with no declaration after it, and only
when the file's Java release is known to be 23 or later (`java.release`, or the build file):
in older code, and when the release is unknown, `///` may be an ordinary comment. A `///` run
inside a method or initializer body is not reported, as `///` there is often an ordinary note
or commented-out code; neither are `////` lines nor `///` comments after code on the same
line.

The rule is off by default: it is part of the `conventions` profile."#,
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
        &["block_comment", "line_comment"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if kind(node) == "line_comment" {
            markdown_run(node, ctx);
            return;
        }
        if !util::is_doc_comment(ctx.text(node)) {
            return;
        }
        let Some(parent) = ctx.parent(node) else {
            return;
        };
        let in_body = match place(parent, node) {
            Place::AfterAnnotations => {
                ctx.emit(node, AFTER_ANNOTATIONS);
                return;
            }
            Place::Body => true,
            Place::Container => false,
            Place::Other => return,
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
                ctx.emit(node, NOTHING_FOLLOWS);
            }
        }
    }
}

const NOTHING_FOLLOWS: &str = "Use a plain comment here, or move it before the declaration it \
                               describes: no declaration follows it";

/// Where a doc comment `node`, a child of `parent`, sits.
enum Place {
    /// After the annotations of a declaration.
    AfterAnnotations,
    /// In a method, constructor, initializer or lambda body, or a `switch` group.
    Body,
    /// In a type body or at the top level of the file.
    Container,
    /// Anywhere else (inside the modifiers before any annotation, inside expressions, …).
    Other,
}

fn place(parent: Node<'_>, node: Node<'_>) -> Place {
    match kind(parent) {
        "modifiers" => {
            // Between annotations and keyword modifiers.
            let after_annotation = syntax::children(parent)
                .take_while(|c| c.id() != node.id())
                .any(|c| matches!(kind(c), "annotation" | "marker_annotation"));
            if after_annotation {
                Place::AfterAnnotations
            } else {
                Place::Other
            }
        }
        "block" | "constructor_body" | "switch_block_statement_group" => Place::Body,
        "class_body"
        | "interface_body"
        | "enum_body"
        | "enum_body_declarations"
        | "annotation_type_body"
        | "program" => Place::Container,
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
                Place::AfterAnnotations
            } else {
                Place::Other
            }
        }
        _ => Place::Other,
    }
}

/// A run of `///` lines (Java 23), reported once, from its first line, and only on a release
/// known to read them as documentation. Inside bodies such runs are not reported: older code
/// and ordinary notes use `///` there too.
fn markdown_run<'t>(line: Node<'t>, ctx: &Ctx<'t>) {
    let src = ctx.source();
    // Cheapest first: the text, the release, then the position.
    if !doc::is_markdown_text(ctx.text(line))
        || !ctx.markdown_docs(DocUse::Reports)
        || !doc::starts_line(src, line)
        || !doc::starts_run(ctx, line)
    {
        return;
    }
    let Some(parent) = ctx.parent(line) else {
        return;
    };
    let in_container = match place(parent, line) {
        Place::AfterAnnotations => false,
        Place::Container => true,
        Place::Body | Place::Other => return,
    };
    let run = doc::run_from(src, parent, line);
    let Some(&last) = run.last() else {
        return;
    };
    if !in_container {
        ctx.emit_range(Span::cover(line, last), AFTER_ANNOTATIONS);
        return;
    }
    match util::next_code(parent, last) {
        Some(next) if is_documentable(next) || kind(next) == "ERROR" => {}
        _ => {
            ctx.emit_range(Span::cover(line, last), NOTHING_FOLLOWS);
        }
    }
}
