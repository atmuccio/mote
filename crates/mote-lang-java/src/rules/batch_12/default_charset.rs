//! `java/default-charset` — hashing text or reading bundled resources with the platform's
//! default charset (before Java 18).

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["charset", "i18n"],
    remediation_minutes: 2,
    description: r#"Detects two conversions that depend on the JVM's default charset although their
result must not depend on the machine, in code compiled for a Java release older than 18:

- text hashed or signed through `getBytes()` without a charset:
  `digest.update(password.getBytes())`, `md.digest(input.getBytes())`, `mac.doFinal(s.getBytes())`;
- a bundled or remote resource read through `new InputStreamReader(…)` without a charset:
  `new InputStreamReader(getClass().getResourceAsStream("/template.txt"))`,
  `new InputStreamReader(url.openStream())`.

**Why it matters.** Up to Java 17 the default charset comes from the operating system and its
settings: UTF-8 on most Linux servers, windows-1252 on many Windows machines. A hash of
non-ASCII text then differs from one machine to the next, so stored password hashes, cache
keys or signatures stop matching when the application moves. A resource file has one fixed
encoding (usually UTF-8), so decoding it with whatever the machine uses garbles its non-ASCII
characters on some installations only.

**How to fix.** Name the charset: `getBytes(StandardCharsets.UTF_8)`,
`new InputStreamReader(in, StandardCharsets.UTF_8)`.

```java
// Noncompliant
byte[] hash = sha256.digest(password.getBytes());

// Compliant
byte[] hash = sha256.digest(password.getBytes(StandardCharsets.UTF_8));
```

Only reported when the Java release is known and below 18: from Java 18 on the default charset
is UTF-8 everywhere (JEP 400) unless explicitly overridden. Other default-charset conversions
(`new String(bytes)`, `FileWriter`, …) are not reported: the platform charset is often the
right one for console, process or local-file text, and that cannot be told from the code.
`getBytes()` on an ASCII string literal and test code are skipped."#,
    ..RuleMeta::new(
        "java/default-charset",
        "java",
        "Hash or resource read with the platform default charset",
    )
};

pub struct DefaultCharset;

/// (class, methods) that hash or sign their byte-array argument.
const HASHING: &[(&str, &[&str])] = &[
    ("java.security.MessageDigest", &["digest", "update"]),
    ("javax.crypto.Mac", &["doFinal", "update"]),
    ("java.security.Signature", &["update"]),
];

impl<'t> JavaRule<'t> for DefaultCharset {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation", "object_creation_expression"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["getBytes"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if ctx.java_release().is_none_or(|r| r >= 18) || ctx.is_test() {
            return;
        }
        if kind(node) == "method_invocation" {
            check_get_bytes(node, ctx);
            return;
        }
        let Some(ty) = node.field(Field::Type) else {
            return;
        };
        if !ctx.text(ty).ends_with("InputStreamReader") || syntax::argument_count(node) != 1 {
            return;
        }
        let Some(arg) = syntax::arguments(node).next().map(syntax::strip_parens) else {
            return;
        };
        if kind(arg) != "method_invocation"
            || !matches!(
                ctx.call_name(arg),
                Some("getResourceAsStream" | "getSystemResourceAsStream" | "openStream")
            )
        {
            return;
        }
        if !ctx
            .resolve_type(ty)
            .is_some_and(|t| t.is_class("java.io.InputStreamReader"))
        {
            return;
        }
        ctx.emit(
            ty,
            "Pass the resource's charset to this InputStreamReader (for example StandardCharsets.UTF_8); the platform default differs between machines",
        );
    }
}

fn check_get_bytes<'t>(node: Node<'t>, ctx: &Ctx<'t>) {
    if ctx.call_name(node) != Some("getBytes") || syntax::argument_count(node) != 0 {
        return;
    }
    let Some(recv) = node.field(Field::Object).map(syntax::strip_parens) else {
        return;
    };
    if syntax::is_string_literal(recv) && ctx.text(recv).is_ascii() {
        return;
    }
    // Directly the argument of a hashing call.
    let Some(list) = ctx.parent(node) else { return };
    if kind(list) != "argument_list" {
        return;
    }
    let Some(call) = ctx.parent(list) else { return };
    if kind(call) != "method_invocation" {
        return;
    }
    let Some(name) = ctx.call_name(call) else {
        return;
    };
    let Some(owner) = call
        .field(Field::Object)
        .and_then(|o| declared_class(o, ctx))
    else {
        return;
    };
    let hashing = HASHING
        .iter()
        .any(|(class, methods)| *class == owner && methods.contains(&name));
    if !hashing {
        return;
    }
    if !ctx.type_of(recv).is_some_and(|t| t.is_string()) {
        return;
    }
    let at = node.field(Field::Name).unwrap_or(node);
    ctx.emit(
        at,
        "Name the charset (getBytes(StandardCharsets.UTF_8)); hashing with the platform default gives different results on different machines",
    );
}

/// The fully-qualified name of the declared class of a variable or field (`md`, `this.md`),
/// through the imports; the hashing classes are not in the resolver's JDK table.
fn declared_class<'t>(expr: Node<'t>, ctx: &Ctx<'t>) -> Option<String> {
    let e = syntax::strip_parens(expr);
    let ident = match kind(e) {
        "identifier" => e,
        "field_access" if e.field(Field::Object).is_some_and(|o| kind(o) == "this") => {
            e.field(Field::Field)?
        }
        _ => return None,
    };
    let decl = ctx.declaration_of(ident)?;
    let ty = decl.type_node?;
    if !matches!(kind(ty), "type_identifier" | "scoped_type_identifier") {
        return None;
    }
    ctx.resolve_type_name(ctx.text(ty), ty)
}
