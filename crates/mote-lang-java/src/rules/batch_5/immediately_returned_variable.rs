//! `java/immediately-returned-variable` — `Foo x = …; return x;`.

use super::util::{self, Mods, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["clutter", "redundant"],
    remediation_minutes: 1,
    description: r#"Detects a local variable that is declared with a value and then immediately
returned or thrown by the next statement.

**Why it matters.** The variable adds a name and a line without adding information; the
reader has to check that nothing happens to it in between.

**How to fix.** Return (or throw) the expression directly.

```java
// Noncompliant
String label = prefix + name;
return label;

IllegalStateException error = new IllegalStateException("closed");
throw error;

// Compliant
return prefix + name;

throw new IllegalStateException("closed");
```

Not reported: annotated declarations (for example `@SuppressWarnings("unchecked")` on a cast,
which cannot be placed on a `return`), array initializers, lambdas and method references
(which need the declared type as their target), and primitive variables whose declared type
differs from the value's type (the conversion would change what is returned)."#,
    ..RuleMeta::new(
        "java/immediately-returned-variable",
        "java",
        "Variable returned immediately after its declaration",
    )
};

pub struct ImmediatelyReturnedVariable;

impl ImmediatelyReturnedVariable {
    /// `decl` (a local variable declaration) directly followed by `next`.
    fn pair<'t>(ctx: &Ctx<'t>, decl_stmt: Node<'t>, next: Node<'t>) {
        let what = match kind(next) {
            "return_statement" => "Return",
            "throw_statement" => "Throw",
            _ => return,
        };
        let Some(returned) = util::first_named(next).map(syntax::strip_parens) else {
            return;
        };
        if kind(returned) != "identifier" {
            return;
        }
        let mut decls = syntax::declarators(decl_stmt);
        let (Some(decl), None) = (decls.next(), decls.next()) else {
            return;
        };
        let (Some(name), Some(value)) = (util::fld(decl, F.name), util::fld(decl, F.value)) else {
            return;
        };
        if ctx.text(name) != ctx.text(returned) || util::fld(decl, F.dimensions).is_some() {
            return;
        }
        if Mods::of(decl_stmt).annotated() {
            return;
        }
        let v = syntax::strip_parens(value);
        if matches!(
            kind(v),
            "array_initializer" | "lambda_expression" | "method_reference"
        ) {
            return;
        }
        let Some(ty) = util::fld(decl_stmt, F.ty) else {
            return;
        };
        if matches!(
            kind(ty),
            "integral_type" | "floating_point_type" | "boolean_type"
        ) {
            let declared = ctx.resolve_type(ty);
            let actual = ctx.type_of(value);
            match (declared, actual) {
                (Some(d), Some(a)) if d == a => {}
                _ => return,
            }
        }
        let var = ctx.text(name);
        ctx.emit(
            name,
            format!("{what} the value directly instead of storing it in `{var}` first"),
        );
    }
}

/// The offset of the first code byte at or after `from` (whitespace and comments skipped).
fn skip_trivia(src: &[u8], mut i: usize) -> usize {
    loop {
        while i < src.len() && src[i].is_ascii_whitespace() {
            i += 1;
        }
        if src.get(i) == Some(&b'/') && src.get(i + 1) == Some(&b'/') {
            while i < src.len() && src[i] != b'\n' {
                i += 1;
            }
        } else if src.get(i) == Some(&b'/') && src.get(i + 1) == Some(&b'*') {
            i += 2;
            while i + 1 < src.len() && !(src[i] == b'*' && src[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(src.len());
        } else {
            return i;
        }
    }
}

impl<'t> JavaRule<'t> for ImmediatelyReturnedVariable {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["local_variable_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        // Cheap text test first: is the next statement a `return` / `throw`?
        let src = ctx.source().as_bytes();
        let at = skip_trivia(src, node.end_byte());
        let rest = &src[at.min(src.len())..];
        let keyword_len = if rest.starts_with(b"return") {
            6
        } else if rest.starts_with(b"throw") {
            5
        } else {
            return;
        };
        if rest
            .get(keyword_len)
            .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_' || *c == b'$')
        {
            return;
        }
        let Some(parent) = ctx.parent(node) else {
            return;
        };
        if !matches!(
            kind(parent),
            "block" | "constructor_body" | "switch_block_statement_group"
        ) {
            return;
        }
        let Some(next) = named_children(parent).find(|c| c.start_byte() == at) else {
            return;
        };
        Self::pair(ctx, node, next);
    }
}
