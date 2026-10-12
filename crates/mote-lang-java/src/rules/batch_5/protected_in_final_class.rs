//! `java/protected-in-final-class` — `protected` members of a class that cannot be extended.

use super::util::{self, Mods, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["design", "visibility"],
    remediation_minutes: 1,
    description: r#"Detects `protected` fields, constructors, methods, and nested types declared
in a `final` class or a record.

**Why it matters.** `protected` means "for subclasses (and the package)". A final class has
no subclasses, so the modifier promises an extension point that cannot exist and confuses
readers about the intended visibility.

**How to fix.** Use `private` or package-private visibility (package-private keeps the access
other classes of the package already have).

```java
// Noncompliant
public final class Invoice {
    protected BigDecimal total;
    protected void recompute() { ... }
}

// Compliant
public final class Invoice {
    private BigDecimal total;
    void recompute() { ... }
}
```

Methods annotated `@Override` are not reported (an override cannot reduce visibility), and
neither are methods of a final class that extends another class, since they may override a
protected method without saying so; `finalize` and `clone` are never reported."#,
    ..RuleMeta::new(
        "java/protected-in-final-class",
        "java",
        "Protected member in a final class",
    )
};

pub struct ProtectedInFinalClass;

impl<'t> JavaRule<'t> for ProtectedInFinalClass {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["class_declaration", "record_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let is_record = kind(node) == "record_declaration";
        if !is_record && !Mods::of(node).has("final") {
            return;
        }
        let extends = util::fld(node, F.superclass).is_some();
        let class_name = syntax::name_of(node, ctx.source());
        for m in util::class_members(ctx, node) {
            let mods = Mods::of(m);
            let Some(kw) = mods.keyword("protected") else {
                continue;
            };
            let what = match kind(m) {
                "field_declaration" => "field",
                "constructor_declaration" => "constructor",
                "method_declaration" => {
                    let name = syntax::name_of(m, ctx.source());
                    if extends
                        || mods.has_annotation(ctx.source(), "Override")
                        || matches!(name, "finalize" | "clone")
                    {
                        continue;
                    }
                    "method"
                }
                k if k.ends_with("_declaration") => "nested type",
                _ => continue,
            };
            let owner = if is_record { "record" } else { "final class" };
            ctx.emit(
                kw,
                format!(
                    "Make this {what} private or package-private: {owner} `{class_name}` cannot \
                     be extended"
                ),
            );
        }
    }
}
