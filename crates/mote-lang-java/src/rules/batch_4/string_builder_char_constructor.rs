//! `java/string-builder-char-constructor` — `new StringBuilder('x')` sets a capacity, not text.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["strings"],
    remediation_minutes: 2,
    description: r#"Detects `new StringBuilder(c)` and `new StringBuffer(c)` where `c` is a `char`
(a character literal or a `char` variable).

**Why it matters.** Neither class has a `char` constructor. The `char` is widened to an `int`
and picked up by the *capacity* constructor, so `new StringBuilder('[')` is an **empty**
builder with room for 91 characters. The text the author meant to start with silently goes
missing.

**How to fix.** Pass a `String`, or append the character.

```java
// Noncompliant
StringBuilder sb = new StringBuilder('[');

// Compliant
StringBuilder sb = new StringBuilder("[");
StringBuilder sb2 = new StringBuilder().append('[');
```

Only arguments whose type is provably `char` are reported; an `int` capacity is fine."#,
    ..RuleMeta::new(
        "java/string-builder-char-constructor",
        "java",
        "StringBuilder created from a char",
    )
};

#[derive(Default)]
pub struct StringBuilderCharConstructor {
    builders: util::Gate,
}

impl<'t> JavaRule<'t> for StringBuilderCharConstructor {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["object_creation_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !self.builders.open(ctx, "StringBu") {
            return;
        }
        let Some(ty) = util::fld(node, F::Type) else {
            return;
        };
        let name = ctx.text(ty);
        let simple = name.rsplit('.').next().unwrap_or(name);
        if !matches!(simple, "StringBuilder" | "StringBuffer") {
            return;
        }
        let mut args = util::args(node);
        let (Some(arg), None) = (args.next(), args.next()) else {
            return;
        };
        let arg_is_char = match kind(syntax::strip_parens(arg)) {
            "character_literal" => true,
            "identifier" | "field_access" | "method_invocation" | "array_access" => ctx
                .type_of(arg)
                .is_some_and(|t| t.prim() == Some(Prim::Char)),
            _ => false,
        };
        if !arg_is_char {
            return;
        }
        let Some(fqn) = util::type_fqn(ctx, ty) else {
            return;
        };
        if !matches!(
            fqn.as_str(),
            "java.lang.StringBuilder" | "java.lang.StringBuffer"
        ) {
            return;
        }
        ctx.emit(
            arg,
            format!("Pass a String here: `new {simple}(char)` uses the character code as the initial capacity and starts empty"),
        );
    }
}
