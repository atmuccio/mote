//! `java/empty-source-file` — a `.java` file that declares no type.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Incubating,
    tags: &["unused", "dead-code"],
    remediation_minutes: 2,
    description: r#"Detects Java source files that contain no class, interface, enum, record or
annotation type: only a package line, imports, comments, or nothing at all.

**Why it matters.** Such a file compiles to nothing. It is usually what remains after the
type it held was moved, renamed or commented out, and it misleads whoever searches the
source tree: the file name promises a type that does not exist, and commented-out code in it
suggests it may come back.

**How to fix.** Delete the file (the version history keeps the old content).

```java
// Noncompliant: OrderMapper.java
package com.acme.orders;

import java.util.Map;

// Mapping moved to OrderConverter.
```

Not reported: `package-info.java`, `module-info.java` and other files whose name is not a
Java identifier (templates such as `license.header.java`), which are not meant to hold a type;
compact source files (top-level methods and fields, Java 25); files under a `resources`
folder (inputs of tests or generators, not compiled sources); and files with syntax errors,
where a declaration may have been lost by the parser."#,
    ..RuleMeta::new(
        "java/empty-source-file",
        "java",
        "Source file without any type declaration",
    )
};

pub struct EmptySourceFile;

impl<'t> JavaRule<'t> for EmptySourceFile {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["program"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        // Cheapest test first, from the end of the file: types come after the package line and
        // the imports, so the last node that is not a comment decides (usually in one step).
        let mut cursor = node.walk();
        if cursor.goto_last_child() {
            loop {
                let c = cursor.node();
                if c.is_named() {
                    match kind(c) {
                        "line_comment" | "block_comment" => {}
                        "package_declaration" | "import_declaration" => break,
                        // A type, a module, or the members of a compact source file (Java 25:
                        // `void main() { … }` without a class).
                        _ => return,
                    }
                }
                if !cursor.goto_previous_sibling() {
                    break;
                }
            }
        }
        let path = ctx.path();
        let file = path.rsplit('/').next().unwrap_or(path);
        // `package-info`, `module-info`, templates such as `license.header.java`: a name that
        // is not a Java identifier cannot hold a public type, so the file is not meant to.
        let stem = file.strip_suffix(".java").unwrap_or(file);
        let identifier = stem
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$')
            && stem
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '$');
        if !identifier || path.contains("/resources/") {
            return;
        }
        if node.has_error() {
            return;
        }
        let mut first = None;
        for c in syntax::children(node).filter(|c| c.is_named()) {
            match kind(c) {
                "package_declaration" => {
                    first.get_or_insert(c);
                }
                "import_declaration" | "line_comment" | "block_comment" => {}
                _ => return,
            }
        }
        ctx.emit(
            first.unwrap_or(node),
            "Delete this file or restore its content: it declares no type",
        );
    }
}
