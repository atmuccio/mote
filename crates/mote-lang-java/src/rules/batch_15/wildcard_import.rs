//! `java/wildcard-import` — `import java.util.*;` or `import static org.junit.Assert.*;`.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["conventions", "imports"],
    params: &[ParamSpec {
        name: "allow_static",
        default: ParamDefault::Bool(false),
        description: "Accept static on-demand imports (`import static org.junit.Assert.*;`) and \
                      report type imports only.",
    }],
    remediation_minutes: 2,
    description: r#"Detects on-demand imports, with or without `static`: `import java.util.*;`,
`import static java.lang.Math.*;`.

**Why it matters.** A wildcard import does not say which names a file uses or where they come
from: a reader (or a review tool showing a diff) cannot tell which package `Entry` or `max`
belongs to. It also makes a file fragile: when a new version of a library adds a class with the
same simple name as one from another wildcard-imported package, the file stops compiling with
an ambiguity error, although nothing in it changed. Single-name imports list the dependencies
exactly and are maintained by every IDE.

**How to fix.** Import each type or static member by name.

```java
// Noncompliant
import java.util.*;
import static java.util.Objects.*;

// Compliant
import java.util.List;
import java.util.Map;
import static java.util.Objects.requireNonNull;
```

Parameter `allow_static` accepts static on-demand imports, which some teams allow for
assertion and matcher libraries in tests. The rule is off by default: it is part of the
`conventions` profile. Module imports (`import module java.base;`, Java 25) are a different
declaration, made to import the API of a whole module at once, and are not reported."#,
    ..RuleMeta::new(
        "java/wildcard-import",
        "java",
        "Wildcard (on-demand) import",
    )
};

pub struct WildcardImport;

impl<'t> JavaRule<'t> for WildcardImport {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["import_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let mut name = None;
        let mut star = None;
        let mut is_static = false;
        for c in syntax::children(node) {
            match kind(c) {
                "static" if !c.is_named() => is_static = true,
                "identifier" | "scoped_identifier" => name = Some(c),
                "asterisk" => star = Some(c),
                _ => {}
            }
        }
        let (Some(name), Some(star)) = (name, star) else {
            return;
        };
        if is_static && ctx.param_bool("allow_static") {
            return;
        }
        let what = if is_static { "static member" } else { "type" };
        ctx.emit_range(
            Span::cover(name, star),
            format!(
                "Import each {what} by name instead of `{}.*`",
                ctx.text(name)
            ),
        );
    }
}
