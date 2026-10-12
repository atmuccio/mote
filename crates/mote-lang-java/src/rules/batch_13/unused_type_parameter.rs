//! `java/unused-type-parameter` — a method type parameter used nowhere in the method.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["generics", "unused"],
    remediation_minutes: 2,
    description: r#"Detects a type parameter of a generic method (`<T> void f(..)`) that appears
nowhere else in the method: not in the return type, the parameters, the `throws` clause, the
bounds of other type parameters, or the body.

**Why it matters.** A method type parameter exists to connect types — an argument to the
result, two arguments to each other. One that connects nothing only makes the signature
harder to read and suggests a relationship that does not exist; callers can even pass an
explicit type argument (`util.<String>clear()`) that has no effect. It is often the leftover
of a refactoring.

**How to fix.** Remove the type parameter.

```java
// Noncompliant
static <T> void clear(List<?> items) { items.clear(); }

// Compliant
static void clear(List<?> items) { items.clear(); }
```

Not reported: methods that may override or implement another method (they must repeat the
parent's type parameters): methods with `@Override`, and non-private instance methods of
classes that extend or implement anything, of anonymous classes and of enum constants;
abstract and interface methods without a body; class-level type parameters (phantom type
parameters are a deliberate technique). A mention anywhere in the method, even in a comment,
counts as a use."#,
    ..RuleMeta::new(
        "java/unused-type-parameter",
        "java",
        "Unused method type parameter",
    )
};

pub struct UnusedTypeParameter;

impl<'t> JavaRule<'t> for UnusedTypeParameter {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_declaration", "constructor_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(tparams) = node.field(Field::TypeParameters) else {
            return;
        };
        if node.field(Field::Body).is_none() {
            return;
        }
        let src = ctx.source();
        if syntax::has_annotation(node, src, "Override") {
            return;
        }
        let static_or_private = kind(node) == "constructor_declaration"
            || syntax::has_modifier(node, "static")
            || syntax::has_modifier(node, "private");
        if !static_or_private && may_override(ctx, node) {
            return;
        }
        // The method text outside its type-parameter list.
        let before = &src[node.start_byte()..tparams.start_byte()];
        let after = &src[tparams.end_byte()..node.end_byte()];
        let list = ctx.text(tparams);
        for tp in named_children(tparams) {
            if kind(tp) != "type_parameter" {
                continue;
            }
            let Some(id) = named_children(tp).find(|c| kind(*c) == "type_identifier") else {
                continue;
            };
            let name = ctx.text(id);
            // A missing name (`<>` in broken code) is no parameter to report.
            if name.is_empty() {
                continue;
            }
            if util::contains_word(before, name)
                || util::contains_word(after, name)
                || count_words(list, name) > 1
            {
                continue;
            }
            let what = if kind(node) == "constructor_declaration" {
                "constructor"
            } else {
                "method"
            };
            ctx.emit(
                id,
                format!(
                    "Remove the type parameter `{name}`: the {what} `{}` never uses it",
                    ctx.name_of(node)
                ),
            );
        }
    }
}

/// Whether an instance method could override or implement a method of another type.
fn may_override<'t>(ctx: &Ctx<'t>, m: Node<'t>) -> bool {
    let Some(class) = ctx.enclosing_class(m) else {
        return true;
    };
    match kind(class) {
        "class_declaration" => {
            class.field(Field::Superclass).is_some() || class.field(Field::Interfaces).is_some()
        }
        "enum_declaration" | "record_declaration" => class.field(Field::Interfaces).is_some(),
        "interface_declaration" => syntax::child_of_kind(class, "extends_interfaces").is_some(),
        // Anonymous classes and enum constant bodies.
        _ => true,
    }
}

/// Whole-word occurrences of `word` in `text` (none of an empty word: searching for one never
/// advanced, so `<>` in a method declaration hung the analysis).
fn count_words(text: &str, word: &str) -> usize {
    if word.is_empty() {
        return 0;
    }
    let is_ident = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    let mut n = 0;
    let mut from = 0;
    while let Some(pos) = text[from..].find(word) {
        let start = from + pos;
        let end = start + word.len();
        let before_ok = text[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !is_ident(c));
        let after_ok = text[end..].chars().next().is_none_or(|c| !is_ident(c));
        if before_ok && after_ok {
            n += 1;
        }
        from = end;
    }
    n
}
