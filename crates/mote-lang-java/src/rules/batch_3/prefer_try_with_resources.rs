//! `java/prefer-try-with-resources` — a local resource closed in a `finally` block instead of
//! being managed by try-with-resources.

use super::resource_not_closed_with_try::tracked_resource;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["resources", "modernization"],
    remediation_minutes: 5,
    description: r#"Detects local variables holding a file or database resource (the kinds tracked by
`resource-not-closed-with-try`: file streams, readers and writers, `Files.newInputStream(…)`
and similar, JDBC connections, statements and result sets) that are closed in a `finally`
block — by `close()` or by a helper such as `closeQuietly(in)` — instead of being declared in a
try-with-resources statement.

**Why it matters.** The `finally` version is correct but fragile: the variable must be declared
outside the `try` (often as `null`), every close needs a null check, an exception thrown by one
`close()` skips the ones after it, and an exception from `close()` replaces the one that caused
the failure. Try-with-resources closes every resource in reverse order on every path and keeps
exceptions from `close()` as suppressed exceptions of the original one.

**How to fix.** Declare the resource in the header of a try-with-resources statement and drop
the `finally` cleanup.

```java
// Noncompliant
PreparedStatement ps = null;
try {
    ps = connection.prepareStatement(sql);
    ps.executeUpdate();
} finally {
    if (ps != null) {
        ps.close();
    }
}

// Compliant
try (PreparedStatement ps = connection.prepareStatement(sql)) {
    ps.executeUpdate();
}
```

Not reported: resources that may leave the method (returned, stored in a field or another
variable, passed to a method outside the `finally` block, captured by a lambda or inner class),
variables that sometimes hold something other than a tracked resource (`in = System.in`),
resources that are not closed in a `finally` block at all (a leak, reported by
`resource-not-closed-with-try`), JDBC objects in test code, and a `ResultSet` obtained from a
statement variable (closed together with the statement)."#,
    ..RuleMeta::new(
        "java/prefer-try-with-resources",
        "java",
        "Resource closed in finally instead of try-with-resources",
    )
};

pub struct PreferTryWithResources;

impl<'t> JavaRule<'t> for PreferTryWithResources {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["local_variable_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        for d in syntax::declarators(node) {
            let Some(t) = tracked_resource(node, d, ctx) else {
                continue;
            };
            if !t.closed_in_finally {
                continue;
            }
            let var = ctx.text(t.name);
            ctx.emit(
                t.name,
                format!(
                    "Declare `{var}` ({}) in a try-with-resources statement instead of closing it in a `finally` block",
                    t.what
                ),
            );
        }
    }
}
