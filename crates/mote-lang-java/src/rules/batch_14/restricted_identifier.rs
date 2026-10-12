//! `java/restricted-identifier` — names that newer Java releases reserve, used where they no
//! longer compile.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["portability", "java-upgrade"],
    remediation_minutes: 10,
    description: r#"Detects names that later Java releases restrict, used in a position where
the newer compiler rejects them:

* a class, interface, enum, record, annotation type or type parameter named `var` (restricted
  from Java 10), `yield` (Java 14), `record` (Java 16), `sealed` or `permits` (Java 17);
* an unqualified call of a method named `yield` (`yield()`, `yield(x)`): from Java 14 the
  compiler reads it as a `yield` statement;
* `_` as a variable or parameter name in code compiled for Java 8 or older (an error from
  Java 9 on).

**Why it matters.** The code compiles today and stops compiling when the project moves to a
newer Java release, so the upgrade starts with renaming work in unexpected places (and in
public types, for every caller).

**How to fix.** Rename the type or type parameter; qualify the call (`this.yield()`,
`Thread.yield()`); give the variable a real name.

```java
// Noncompliant
class record { … }
public void run() { yield(); }

// Compliant
class Record { … }
public void run() { Thread.yield(); }
```

Findings depend on the Java release the file is compiled for (from `java.release` in the
configuration, or the build file): a name is only reported when the release is older than the
one that restricts it, or unknown. `_` is only reported when the release is known to be 8 or
older, because Java 22 gave it a new, valid meaning (an unnamed variable). Variables, fields
and methods named `var`, `yield`, `record`, `sealed` or `permits` are not reported: they
remain legal."#,
    ..RuleMeta::new(
        "java/restricted-identifier",
        "java",
        "Identifier restricted in newer Java releases",
    )
};

pub struct RestrictedIdentifier;

const UNDERSCORE: &str = "Rename `_`: from Java 9 it is a keyword and this code no longer compiles";

/// The release that restricts `name` as a type name.
fn type_name_release(name: &str) -> Option<u32> {
    Some(match name {
        "var" => 10,
        "yield" => 14,
        "record" => 16,
        "sealed" | "permits" => 17,
        _ => return None,
    })
}

/// Whether the file's release is older than `since` (or unknown).
fn older_than(ctx: &Ctx<'_>, since: u32) -> bool {
    ctx.java_release().is_none_or(|r| r < since)
}

impl RestrictedIdentifier {
    fn type_name<'t>(ctx: &Ctx<'t>, name: Node<'t>, what: &str) {
        let text = ctx.text(name);
        let Some(since) = type_name_release(text) else {
            return;
        };
        if !older_than(ctx, since) {
            return;
        }
        ctx.emit(
            name,
            format!(
                "Rename the {what} `{text}`: `{text}` is a restricted type name from Java {since}, where this declaration no longer compiles"
            ),
        );
    }
}

impl<'t> JavaRule<'t> for RestrictedIdentifier {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "class_declaration",
            "interface_declaration",
            "enum_declaration",
            "record_declaration",
            "annotation_type_declaration",
            "type_parameter",
            "method_invocation",
            "yield_statement",
            "underscore_pattern",
            "lambda_expression",
        ]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["yield"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        match kind(node) {
            "lambda_expression" => {
                // `_ -> ..` and `(_, y) -> ..`: the parameters are plain identifiers.
                let Some(params) = node.field(Field::Parameters) else {
                    return;
                };
                if !ctx.text(params).contains('_') || ctx.java_release().is_none_or(|r| r > 8) {
                    return;
                }
                let underscore = |n: Node<'t>| kind(n) == "identifier" && ctx.text(n) == "_";
                let found = if underscore(params) {
                    Some(params)
                } else {
                    named_children(params).find(|p| underscore(*p))
                };
                if let Some(u) = found {
                    ctx.emit(u, UNDERSCORE);
                }
            }
            "underscore_pattern" => {
                if ctx.java_release().is_some_and(|r| r <= 8) {
                    ctx.emit(node, UNDERSCORE);
                }
            }
            "type_parameter" => {
                if let Some(name) = syntax::first_named_child(node) {
                    if kind(name) == "type_identifier" {
                        Self::type_name(ctx, name, "type parameter");
                    }
                }
            }
            "method_invocation" => {
                if ctx.call_name(node) != Some("yield") || node.field(Field::Object).is_some() {
                    return;
                }
                if !older_than(ctx, 14) {
                    return;
                }
                let Some(name) = node.field(Field::Name) else {
                    return;
                };
                ctx.emit(
                    name,
                    "Qualify this call (`this.yield(..)`, `Thread.yield()`): from Java 14 an unqualified `yield` starts a yield statement",
                );
            }
            "yield_statement" => {
                // `yield(x);` outside any switch is a call of a method named `yield`, parsed as
                // a yield statement.
                let Some(value) = syntax::first_named_child(node) else {
                    return;
                };
                if kind(value) != "parenthesized_expression" || !older_than(ctx, 14) {
                    return;
                }
                for a in ctx.ancestors(node) {
                    if kind(a) == "switch_block" {
                        return;
                    }
                    if syntax::is_function_like(a) || kind(a) == "class_body" {
                        break;
                    }
                }
                ctx.emit_range(
                    Span::new(node.start_byte(), value.start_byte()),
                    "Qualify this call (`this.yield(..)`, `Thread.yield()`): from Java 14 an unqualified `yield` starts a yield statement",
                );
            }
            _ => {
                let Some(name) = node.field(Field::Name) else {
                    return;
                };
                let what = match kind(node) {
                    "interface_declaration" => "interface",
                    "enum_declaration" => "enum",
                    "record_declaration" => "record",
                    "annotation_type_declaration" => "annotation type",
                    _ => "class",
                };
                Self::type_name(ctx, name, what);
            }
        }
    }
}
