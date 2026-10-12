//! `java/multiple-top-level-types` — a source file declaring more than one top-level type.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["conventions", "structure"],
    params: &[ParamSpec {
        name: "allow_package_private",
        default: ParamDefault::Bool(false),
        description: "Accept package-private helper types next to the file's public type. A \
                      file without a public type is still reported.",
    }],
    remediation_minutes: 5,
    description: r#"Detects source files that declare more than one top-level class, interface,
enum, record or annotation type.

**Why it matters.** Tools and people find a type by its file: `Order.java` holds `Order`. A
second top-level type in that file is invisible from the file list, cannot be found by the
usual "open type by file name" habits, and makes the compiler's output (one `.class` file per
type) disagree with the source tree. When two files both declare a package-private type of the
same name, the build fails depending on which files are compiled together.

**How to fix.** Move the extra type to its own file, or nest it inside the main type
(`static` when it does not need an instance of the outer class).

```java
// Noncompliant: Order.java
public class Order { … }
class OrderLine { … }

// Compliant: Order.java
public class Order {
    static class Line { … }
}
```

The type kept in place is the one named after the file, else the public one, else the first;
every other top-level type is reported. Parameter `allow_package_private` accepts
package-private helper types next to a public type, a layout older conventions tolerate.
Files whose name is not a Java identifier (templates) and files under a `resources` directory
are not checked. The rule is off by default: it is part of the `conventions` profile."#,
    ..RuleMeta::new(
        "java/multiple-top-level-types",
        "java",
        "Several top-level types in one file",
    )
};

pub struct MultipleTopLevelTypes;

impl<'t> JavaRule<'t> for MultipleTopLevelTypes {
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
        if types.len() < 2 {
            return;
        }
        let is_public = |t: Node<'t>| syntax::has_modifier(t, "public");
        let main = types
            .iter()
            .position(|t| ctx.name_of(*t) == stem)
            .or_else(|| types.iter().position(|t| is_public(*t)))
            .unwrap_or(0);
        let has_public = is_public(types[main]);
        for (i, t) in types.iter().enumerate() {
            if i == main {
                continue;
            }
            if has_public && !is_public(*t) && ctx.param_bool("allow_package_private") {
                continue;
            }
            let at = ctx.name_node(*t).unwrap_or(*t);
            ctx.emit(
                at,
                format!(
                    "Move `{}` to its own file, or nest it in `{}`",
                    ctx.text(at),
                    ctx.name_of(types[main])
                ),
            );
        }
    }
}
