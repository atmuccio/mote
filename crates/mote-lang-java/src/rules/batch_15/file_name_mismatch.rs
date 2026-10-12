//! `java/file-name-mismatch` — the only top-level type of a file is named differently from the
//! file.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Incubating,
    default_enabled: false,
    tags: &["conventions", "structure"],
    remediation_minutes: 2,
    description: r#"Detects a source file whose only top-level type has a different name than
the file (`Helpers.java` declaring `class Utils`), compared case-sensitively.

**Why it matters.** The compiler insists on a matching name only for public types; a
package-private type can live in any file. Everyone who looks for `Utils` opens `Utils.java`,
which does not exist, and a later rename of the file or the type to "fix" the visibility then
breaks in surprising ways. On case-insensitive file systems a difference in case alone
(`Userservice.java` holding `UserService`) behaves differently from one machine to the next.

**How to fix.** Rename the file after the type, or the type after the file.

```java
// Noncompliant: Helpers.java
final class Utils { … }

// Compliant: Utils.java
final class Utils { … }
```

Not reported: files with several top-level types (see `java/multiple-top-level-types`),
`package-info.java`, `module-info.java`, compact source files without a class, files whose
name is not a Java identifier (templates), and files under a `resources` directory. The rule
is off by default: it is part of the `conventions` profile."#,
    ..RuleMeta::new(
        "java/file-name-mismatch",
        "java",
        "Top-level type named differently from its file",
    )
};

pub struct FileNameMismatch;

impl<'t> JavaRule<'t> for FileNameMismatch {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["program"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let path = ctx.path();
        let Some(stem) = util::type_like_file_stem(path) else {
            return;
        };
        if path.contains("/resources/") || path.starts_with("resources/") || node.has_error() {
            return;
        }
        let Some(types) = util::top_level_types(node) else {
            return;
        };
        let [decl] = types[..] else {
            return;
        };
        let Some(name) = ctx.name_node(decl) else {
            return;
        };
        let type_name = ctx.text(name);
        if type_name == stem || type_name.is_empty() {
            return;
        }
        ctx.emit(
            name,
            format!("Rename the file to `{type_name}.java`, or the type to `{stem}`"),
        );
    }
}
