//! `java/default-package` — types declared without a `package` statement.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Incubating,
    tags: &["structure", "convention"],
    remediation_minutes: 5,
    description: r#"Detects a source file that declares types but has no `package` statement, so
its types live in the unnamed (default) package.

**Why it matters.** Types of the unnamed package cannot be imported by any class of a named
package, so nothing else in the code base can use them without reflection. Their simple names
are global and clash with any other default-package class on the class path, the module
system does not allow them in named modules, and tools that organize code by package (access
rules, architecture checks, coverage and logging configuration) cannot address them.

**How to fix.** Add a `package` declaration that matches the file's directory, and move the
file if needed.

```java
// Noncompliant (src/main/java/InvoiceParser.java)
public class InvoiceParser { … }

// Compliant (src/main/java/com/acme/billing/InvoiceParser.java)
package com.acme.billing;

public class InvoiceParser { … }
```

Not reported: a file with a `main` method (a program meant to be run directly, for instance
with the source launcher), test code (tests of how tools handle the unnamed package use it on
purpose), files at the root of the scanned tree, files under `resources`, test-data,
template and example directories (sources used as input data, and samples meant to be
compiled on their own), `package-info.java` and `module-info.java`, and files with syntax
errors."#,
    ..RuleMeta::new(
        "java/default-package",
        "java",
        "Type in the default package",
    )
};

#[derive(Default)]
pub struct DefaultPackage<'t> {
    has_package: bool,
    /// Top-level type declarations.
    types: Vec<Node<'t>>,
}

/// Directory names that hold data or templates rather than compiled sources.
const DATA_DIRS: &[&str] = &[
    "resources",
    "testdata",
    "test-data",
    "testData",
    "fixtures",
    "testfiles",
    "test-files",
    "templates",
    "template",
    "generator",
    "examples",
    "example",
    "samples",
    "sample",
    "demo",
    "demos",
];

impl<'t> JavaRule<'t> for DefaultPackage<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "package_declaration",
            "class_declaration",
            "interface_declaration",
            "enum_declaration",
            "record_declaration",
            "annotation_type_declaration",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if kind(node) == "package_declaration" {
            self.has_package = true;
            return;
        }
        if !self.has_package && ctx.parent(node).is_some_and(|p| kind(p) == "program") {
            self.types.push(node);
        }
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        let Some(&first) = self.types.first() else {
            return;
        };
        if self.has_package || ctx.is_test() || ctx.root().has_error() {
            return;
        }
        let path = ctx.path();
        let mut dirs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let file = dirs.pop().unwrap_or("");
        if dirs.is_empty()
            || dirs.iter().any(|d| DATA_DIRS.contains(d))
            || matches!(file, "package-info.java" | "module-info.java")
        {
            return;
        }
        if self.types.iter().any(|t| has_main(ctx, *t)) {
            return;
        }
        let Some(name) = first.field(Field::Name) else {
            return;
        };
        ctx.emit(
            name,
            format!(
                "Declare a package for `{}`: types in the default package cannot be imported from named packages",
                ctx.text(name)
            ),
        );
    }
}

/// Whether a type declares a `main` method.
fn has_main<'t>(ctx: &Ctx<'t>, decl: Node<'t>) -> bool {
    let Some(body) = decl.field(Field::Body) else {
        return false;
    };
    let src = ctx.source();
    ctx.members(body)
        .any(|m| kind(m) == "method_declaration" && syntax::name_of(m, src) == "main")
}
