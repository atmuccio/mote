//! `java/double-brace-initialization` — `new HashMap<>() {{ put(..); }}`.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["collections", "memory"],
    remediation_minutes: 5,
    description: r#"Detects "double-brace initialization": an anonymous subclass of a collection
or map whose instance initializer fills it (`new ArrayList<>() {{ add(..); }}`).

**Why it matters.** Each occurrence creates a new class. Inside an instance method the
anonymous class also captures the enclosing object, so the collection keeps its creator alive
(a classic memory leak when the collection is cached or returned), and serializing it drags
the outer object along — or fails. The subclass also breaks `equals` symmetry with code that
compares classes and surprises serialization frameworks.

**How to fix.** Build the collection with ordinary statements, or use a factory method.

```java
// Noncompliant
Map<String, Integer> limits = new HashMap<>() {{
    put("users", 100);
    put("roles", 10);
}};

// Compliant
Map<String, Integer> limits = new HashMap<>();
limits.put("users", 100);
limits.put("roles", 10);
// or, when an unmodifiable map is fine:
Map<String, Integer> limits = Map.of("users", 100, "roles", 10);
```

Only anonymous classes whose body holds nothing but initializer blocks (and possibly a
`serialVersionUID`) are reported, and only when the created type is known to be a
`java.util.Collection` or `java.util.Map`."#,
    ..RuleMeta::new(
        "java/double-brace-initialization",
        "java",
        "Double-brace initialization",
    )
};

pub struct DoubleBraceInitialization;

impl<'t> JavaRule<'t> for DoubleBraceInitialization {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["object_creation_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !ctx.text(node).ends_with('}') {
            return;
        }
        let Some(body) = util::anonymous_body(node) else {
            return;
        };
        let mut initializers = 0;
        for m in named_children(body) {
            match kind(m) {
                "block" => initializers += 1,
                "field_declaration"
                    if syntax::declarators(m)
                        .all(|d| syntax::name_of(d, ctx.source()) == "serialVersionUID") => {}
                _ => return,
            }
        }
        if initializers == 0 {
            return;
        }
        let Some(ty) = util::fld(node, F.ty) else {
            return;
        };
        let Some(t) = ctx.resolve_type(ty) else {
            return;
        };
        let what = if ctx.is_subtype_of(&t, "java.util.Map") {
            "map"
        } else if ctx.is_subtype_of(&t, "java.util.Collection") {
            "collection"
        } else {
            return;
        };
        ctx.emit(
            ty,
            format!(
                "Fill this {what} with plain statements or a factory method instead of \
                 double-brace initialization"
            ),
        );
    }
}
