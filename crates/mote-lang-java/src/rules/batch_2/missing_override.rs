//! `java/missing-override` — overriding methods without `@Override`.

use super::util::Mods;
use crate::prelude::*;
use crate::resolve::OverrideIndex;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["convention", "overrides"],
    remediation_minutes: 1,
    description: r#"Detects methods that override or implement a method of a supertype without being
annotated with `@Override`.

**Why it matters.** `@Override` makes the compiler verify that the method really overrides
the inherited one. Without it, a slip such as `equals(Point other)`, `hashcode()`, or a changed
parameter type in the supertype compiles fine as an unrelated new method, and the inherited
behaviour silently stays in place. The annotation also tells readers at a glance that the
method is part of a contract.

**How to fix.** Add `@Override`.

```java
// Noncompliant
class Job implements Runnable {
    public void run() { work(); }
}

// Compliant
class Job implements Runnable {
    @Override
    public void run() { work(); }
}
```

The supertype must be known: a class or interface declared in the same file, or a common JDK
type (`Object`, `Runnable`, `Callable`, `Comparable`, `Comparator`, `Iterator`, `Iterable`,
`AutoCloseable`, `Thread`, `Throwable`, collection and map types, functional interfaces, I/O
streams, AWT/Swing listeners, SAX handlers, the JDBC and DOM interfaces, …). The method must
match by name, number of parameters, and parameter types. Methods overriding supertypes from other files or libraries
are not reported, nor are methods without a body (abstract or interface declarations), static
and private methods, and methods declared in interfaces (including `default` methods).

**Fix available.** `mote fix` adds `@Override` (on its own line, indented like the method) to
the methods the rule reports — those it has matched with a method of a known supertype. The
annotation is checked by the compiler and not kept in the class file, so the program does the
same; it is not added where the file imports or declares another type named `Override`."#,
    fixable: true,
    ..RuleMeta::new(
        "java/missing-override",
        "java",
        "Missing @Override on an overriding method",
    )
};

/// Per file: the override index of each class seen so far (built once per class).
#[derive(Default)]
pub struct MissingOverride<'t> {
    /// Override indexes of the classes seen, by node id (a list searched per method was
    /// quadratic in an enum of thousands of constants with bodies).
    indexes: FxHashMap<usize, Option<OverrideIndex<'t>>>,
    /// Whether the file imports or declares a type named `Override` (asked once per file, when
    /// the first finding gets its fix).
    override_shadowed: Option<bool>,
}

/// A parameter type as written, without type arguments, annotations, or package:
/// `java.util.Map.Entry<K, V>` → `Entry`, `String...` stays `String...`.
fn short_type(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut depth = 0u32;
    for c in text.chars() {
        match c {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    let last = out.split_whitespace().last().unwrap_or("");
    let base = last.trim_end_matches(['[', ']', '.']);
    let suffix = &last[base.len()..];
    let simple = base.rsplit('.').next().unwrap_or(base);
    format!("{simple}{suffix}")
}

impl<'t> JavaRule<'t> for MissingOverride<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) = ctx.name_node(node) else {
            return;
        };
        // Inside a class, enum, record, or anonymous class (not an interface); the parent comes
        // from the walk stack, so this is the cheapest test.
        match ctx.parent(node).map(kind) {
            Some("class_body" | "enum_body_declarations") => {}
            _ => return,
        }
        let mods = Mods::of(node);
        if node.field(Field::Body).is_none() || mods.has("static") || mods.has("private") {
            return;
        }
        if mods.has_annotation(ctx.source(), "Override") {
            return;
        }
        let Some(owner) = ctx.enclosing_class(node) else {
            return;
        };
        let overrides = self
            .indexes
            .entry(owner.id())
            .or_insert_with(|| ctx.override_index(owner))
            .as_ref()
            .is_some_and(|index| ctx.overrides_known_method(node, index));
        if !overrides {
            return;
        }
        let params: Vec<String> = syntax::parameters(node)
            .map(|p| {
                if kind(p) == "spread_parameter" {
                    let ty = syntax::named_children(p)
                        .find(|c| !matches!(kind(*c), "modifiers" | "variable_declarator"))
                        .map_or("", |t| ctx.text(t));
                    format!("{}...", short_type(ty))
                } else {
                    short_type(p.field(Field::Type).map_or("", |t| ctx.text(t)))
                }
            })
            .collect();
        let sig = format!("{}({})", ctx.text(name), params.join(", "));
        let emitted = ctx.emit(name, format!("Add @Override to `{sig}`"));
        // `Override` must mean `java.lang.Override`: not imported or declared otherwise here.
        let shadowed = *self.override_shadowed.get_or_insert_with(|| {
            ctx.imports()
                .iter()
                .any(|i| !i.is_wildcard && i.simple_name() == "Override")
                || declares_type(ctx.source(), "Override")
        });
        if !shadowed {
            emitted.fix("Add `@Override`", vec![add_override(ctx.source(), node)]);
        }
    }
}

/// Whether `src` declares a type named `name` (`class Override`, `@interface Override`, …).
fn declares_type(src: &str, name: &str) -> bool {
    src.match_indices(name).any(|(at, _)| {
        let before = src[..at].trim_end();
        let word = before
            .rsplit(|c: char| !c.is_ascii_alphanumeric() && c != '@')
            .next()
            .unwrap_or("");
        before.len() < at
            && matches!(
                word,
                "class" | "interface" | "@interface" | "enum" | "record"
            )
    })
}

/// Insert `@Override` in front of `method`: on a line of its own, indented like the method,
/// when the method starts its line; else on the same line.
fn add_override(src: &str, method: Node<'_>) -> Edit {
    let at = method.start_byte();
    let line_start = src[..at].rfind('\n').map_or(0, |i| i + 1);
    let indent = &src[line_start..at];
    if indent.bytes().all(|b| b == b' ' || b == b'\t') {
        let nl = fix::line_break_at(src, at);
        Edit::insert(at, format!("@Override{nl}{indent}"))
    } else {
        Edit::insert(at, "@Override ")
    }
}

#[cfg(test)]
mod tests {
    use super::short_type;

    #[test]
    fn short_type_strips_packages_and_arguments() {
        assert_eq!(short_type("java.lang.Object"), "Object");
        assert_eq!(short_type("Map.Entry<String, List<Integer>>"), "Entry");
        assert_eq!(short_type("int[]"), "int[]");
        assert_eq!(short_type("String..."), "String...");
        assert_eq!(short_type("java.lang.String..."), "String...");
        assert_eq!(short_type("@NonNull String"), "String");
    }
}
