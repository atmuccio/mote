//! `java/package-directory-mismatch` — a package declaration that does not match the directory
//! of the file.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Incubating,
    tags: &["structure", "convention"],
    remediation_minutes: 5,
    description: r#"Detects a `package` declaration whose names do not match the last directories of
the file's path: `package com.acme.billing;` in `src/main/java/com/acme/orders/Invoice.java`.

**Why it matters.** Build tools, IDEs and the compiler's source path look for a class of
package `com.acme.billing` under `com/acme/billing/`. A file elsewhere compiles only when it
is passed explicitly, is not found by incremental builds and by tools that locate sources
from class names (debuggers, coverage reports, navigation), and misleads readers who browse
the directory tree.

**How to fix.** Move the file to the directory of its package, or correct the declaration.

Only the directories the path shows are compared (a scan of a sub-directory sees a shortened
path). Not reported: files at the root of the scanned tree, files under `resources`
directories or test-data directories (sources used as input data), and package names that are
template placeholders."#,
    ..RuleMeta::new(
        "java/package-directory-mismatch",
        "java",
        "Package does not match directory",
    )
};

pub struct PackageDirectoryMismatch;

/// Directory names that hold data rather than compiled sources.
const DATA_DIRS: &[&str] = &[
    "resources",
    "testdata",
    "test-data",
    "testData",
    "fixtures",
    "testfiles",
    "test-files",
];

impl<'t> JavaRule<'t> for PackageDirectoryMismatch {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["package_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) =
            named_children(node).find(|c| matches!(kind(*c), "scoped_identifier" | "identifier"))
        else {
            return;
        };
        let pkg: String = ctx
            .text(name)
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        if pkg.is_empty()
            || !pkg
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '.')
        {
            return;
        }
        let path = ctx.path();
        let mut dirs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        dirs.pop(); // the file name
        if dirs.is_empty() || dirs.iter().any(|d| DATA_DIRS.contains(d)) {
            return;
        }
        let segs: Vec<&str> = pkg.split('.').collect();
        let n = segs.len().min(dirs.len());
        let pkg_tail = &segs[segs.len() - n..];
        let dir_tail = &dirs[dirs.len() - n..];
        if pkg_tail == dir_tail {
            return;
        }
        let expected = segs.join("/");
        ctx.emit(
            name,
            format!(
                "Move this file to a `{expected}` directory, or change the package: it is in `{}`",
                dirs.join("/")
            ),
        );
    }
}
