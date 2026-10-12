//! `java/unused-private-method-parameter` — a parameter of a private method that the body
//! never mentions.

use super::util::{self, Mods, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["dead-code", "unused"],
    remediation_minutes: 5,
    description: r##"Detects parameters of `private` methods that the method body never uses.

**Why it matters.** A private method is only called from its own file, so its signature is
entirely under local control. An unused parameter makes every caller compute and pass a value
for nothing, suggests a dependency that doesn't exist, and often reveals a bug: the value was
meant to be used (a wrong variable was used instead).

**How to fix.** Remove the parameter and the corresponding argument at every call site — or
use it, if it was forgotten.

```java
// Noncompliant
private String label(Order order, Locale locale) {
    return "#" + order.id();              // locale is never used
}

// Compliant
private String label(Order order) {
    return "#" + order.id();
}
```

A parameter counts as used when its name appears anywhere in the code of the body
(conservative), but not inside a string literal or a comment (`"line.separator"` does not use
`line`).
Not reported: `_` and names starting with `unused` or `ignored`, annotated methods and
parameters (framework callbacks, `@SuppressWarnings("unused")`), methods with an empty body,
the serialization hooks (`readObject`, `writeObject`, …), and methods that are referenced as
a method reference (`this::name`) or by name in a string (reflection), where the signature
must stay as it is."##,
    ..RuleMeta::new(
        "java/unused-private-method-parameter",
        "java",
        "Unused parameter of a private method",
    )
};

const SERIALIZATION_HOOKS: &[&str] = &[
    "writeObject",
    "readObject",
    "readObjectNoData",
    "writeReplace",
    "readResolve",
];

pub struct UnusedPrivateMethodParameter;

fn intentionally_unused(name: &str) -> bool {
    let lower = |p: &str| {
        name.len() >= p.len() && name.as_bytes()[..p.len()].eq_ignore_ascii_case(p.as_bytes())
    };
    name == "_" || lower("unused") || lower("ignored")
}

/// `::name` (method reference) or `"name"` (reflection) anywhere in the file.
fn referenced_indirectly(ctx: &Ctx<'_>, name: &str) -> bool {
    let src = ctx.source();
    let b = src.as_bytes();
    util::source_words(ctx, name, 0, src.len()).any(|at| {
        let before = src.get(..at).unwrap_or("").trim_end();
        before.ends_with("::")
            || (at > 0 && b.get(at - 1) == Some(&b'"') && b.get(at + name.len()) == Some(&b'"'))
    })
}

/// Whether `name` occurs in `body` as code: a mention inside a string literal or a comment is
/// not a use.
fn used_in<'t>(ctx: &Ctx<'t>, name: &str, body: Node<'t>) -> bool {
    let (start, end) = (body.start_byte(), body.end_byte());
    util::source_words(ctx, name, start, end).any(|at| {
        // A word inside `body` (which starts and ends with a brace): the descent from the root
        // passes through `body`, so start there.
        body.descendant_for_byte_range(at, at + name.len())
            .is_none_or(|n| {
                !syntax::is_comment(n)
                    && !matches!(
                        kind(n),
                        "string_fragment" | "string_literal" | "escape_sequence"
                    )
            })
    })
}

impl<'t> JavaRule<'t> for UnusedPrivateMethodParameter {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let mods = Mods::of(node);
        if !mods.has("private") || mods.annotated() {
            return;
        }
        let Some(body) = util::fld(node, F.body) else {
            return;
        };
        if syntax::is_empty_block(body) {
            return;
        }
        let method = ctx.name_of(node);
        if SERIALIZATION_HOOKS.contains(&method) {
            return;
        }
        let mut checked_references = false;
        for p in syntax::parameters(node) {
            let name_node = match kind(p) {
                "formal_parameter" => util::fld(p, F.name),
                // `String... args`: the name sits in a variable_declarator.
                _ => named_children(p)
                    .find(|c| kind(*c) == "variable_declarator")
                    .and_then(|d| util::fld(d, F.name)),
            };
            let Some(name_node) = name_node else {
                continue;
            };
            if kind(name_node) != "identifier" || Mods::of(p).annotated() {
                continue;
            }
            let name = ctx.text(name_node);
            if intentionally_unused(name) || used_in(ctx, name, body) {
                continue;
            }
            if !checked_references {
                if referenced_indirectly(ctx, method) {
                    return;
                }
                checked_references = true;
            }
            ctx.emit(
                name_node,
                format!("Remove the unused parameter `{name}` of private method `{method}`"),
            );
        }
    }
}
