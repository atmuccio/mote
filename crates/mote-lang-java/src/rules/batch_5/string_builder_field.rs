//! `java/string-builder-field` — a `StringBuilder` / `StringBuffer` kept in a field.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["design", "memory"],
    remediation_minutes: 10,
    description: r#"Detects fields of type `StringBuilder` or `StringBuffer`.

**Why it matters.** A builder is a scratch buffer: it is meant to live while one string is
assembled. Kept in a field it holds on to its (possibly large) internal array for the whole
life of the object, its contents leak from one use to the next unless every path remembers to
reset it, and a shared mutable buffer is unsafe to use from several threads (and even
`StringBuffer`'s synchronization does not make multi-call sequences atomic).

**How to fix.** Create the builder as a local variable where the string is assembled, or keep
the finished `String` (or a list of parts) in the field instead.

```java
// Noncompliant
class ReportWriter {
    private final StringBuilder buffer = new StringBuilder();
    String render(Report r) {
        buffer.append(r.title());
        return buffer.toString();       // grows with every call
    }
}

// Compliant
class ReportWriter {
    String render(Report r) {
        StringBuilder buffer = new StringBuilder();
        buffer.append(r.title());
        return buffer.toString();
    }
}
```

This is a design guideline: classes whose whole purpose is to accumulate text (writers,
code generators) may keep a builder deliberately; suppress the finding there. The rule is
disabled by default."#,
    default_enabled: false,
    ..RuleMeta::new(
        "java/string-builder-field",
        "java",
        "StringBuilder kept in a field",
    )
};

pub struct StringBuilderField;

impl<'t> JavaRule<'t> for StringBuilderField {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["field_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(ty) = util::fld(node, F.ty) else {
            return;
        };
        if !matches!(kind(ty), "type_identifier" | "scoped_type_identifier") {
            return;
        }
        let text = ctx.text(ty);
        let simple = text.rsplit('.').next().unwrap_or(text);
        if simple != "StringBuilder" && simple != "StringBuffer" {
            return;
        }
        let Some(fqn) = util::type_fqn(ctx, ty) else {
            return;
        };
        if fqn != "java.lang.StringBuilder" && fqn != "java.lang.StringBuffer" {
            return;
        }
        let names: Vec<&str> = syntax::declarators(node)
            .filter(|d| util::fld(*d, F.dimensions).is_none())
            .filter_map(|d| util::fld(d, F.name))
            .map(|n| ctx.text(n))
            .collect();
        let Some(first) = names.first() else {
            return;
        };
        ctx.emit(
            ty,
            format!("Use a local `{simple}` instead of keeping one in field `{first}`"),
        );
    }
}
