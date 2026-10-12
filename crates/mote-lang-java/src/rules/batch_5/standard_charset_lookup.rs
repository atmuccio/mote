//! `java/standard-charset-lookup` — `Charset.forName("UTF-8")`, `getBytes("UTF-8")`.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["charset", "clarity"],
    remediation_minutes: 2,
    description: r#"Detects charsets that `java.nio.charset.StandardCharsets` provides being
looked up by name: `Charset.forName("UTF-8")`, `s.getBytes("UTF-8")`,
`new String(bytes, "UTF-8")`, `new InputStreamReader(in, "UTF-8")`,
`new OutputStreamWriter(out, "UTF-8")`, and — on Java 10 and later, which added the `Charset`
overloads — `URLEncoder.encode(s, "UTF-8")` and `URLDecoder.decode(s, "UTF-8")`.

**Why it matters.** A name is only checked at run time — a typo becomes an exception in
production — and the `String`-based overloads declare the checked
`UnsupportedEncodingException`, which forces pointless `try`/`catch` blocks for charsets every
JVM is required to support. The constants are also slightly faster (no lookup).

**How to fix.** Use the constant: `StandardCharsets.UTF_8`, `US_ASCII`, `ISO_8859_1`,
`UTF_16`, `UTF_16BE`, or `UTF_16LE`, and drop the now-unneeded exception handling.

```java
// Noncompliant
byte[] data = text.getBytes("UTF-8");
Charset latin = Charset.forName("ISO-8859-1");

// Compliant
byte[] data = text.getBytes(StandardCharsets.UTF_8);
Charset latin = StandardCharsets.ISO_8859_1;
```

Only string literals naming one of the six standard charsets (or a common alias such as
`"UTF8"`) are reported — for `URLEncoder` / `URLDecoder` also a `static final` constant of this
file holding such a literal; other charsets and names held in variables are left alone."#,
    ..RuleMeta::new(
        "java/standard-charset-lookup",
        "java",
        "Standard charset looked up by name",
    )
};

/// The `StandardCharsets` constant for a charset name, if there is one.
fn standard_constant(name: &str) -> Option<&'static str> {
    let upper = name.to_ascii_uppercase();
    Some(match upper.as_str() {
        "UTF-8" | "UTF8" => "UTF_8",
        "US-ASCII" | "ASCII" => "US_ASCII",
        "ISO-8859-1" | "ISO8859-1" | "ISO8859_1" | "ISO_8859_1" | "LATIN1" => "ISO_8859_1",
        "UTF-16" | "UTF16" | "UTF_16" => "UTF_16",
        "UTF-16BE" | "UTF_16BE" => "UTF_16BE",
        "UTF-16LE" | "UTF_16LE" => "UTF_16LE",
        _ => return None,
    })
}

pub struct StandardCharsetLookup;

/// The literal value of a `static final` field of this file that `ident` names.
fn constant_literal<'t>(ctx: &Ctx<'t>, ident: Node<'t>) -> Option<Node<'t>> {
    if kind(ident) != "identifier" {
        return None;
    }
    let d = ctx.declaration_of(ident)?;
    if d.kind != VarKind::Field {
        return None;
    }
    let constant = kind(d.statement) == "constant_declaration"
        || (syntax::has_modifier(d.statement, "static")
            && syntax::has_modifier(d.statement, "final"));
    let value = syntax::strip_parens(util::fld(d.decl, F.value)?);
    (constant && syntax::is_string_literal(value)).then_some(value)
}

impl StandardCharsetLookup {
    /// `URLEncoder.encode(s, "UTF-8")` / `URLDecoder.decode(s, CHARSET)` on Java 10+.
    fn url_coding<'t>(ctx: &Ctx<'t>, node: Node<'t>, name: &str) {
        if util::arg_count(node) != 2 || ctx.java_release().is_none_or(|r| r < 10) {
            return;
        }
        let owner = if name == "encode" {
            "java.net.URLEncoder"
        } else {
            "java.net.URLDecoder"
        };
        if util::fld(node, F.object)
            .and_then(|o| ctx.type_name_of(o))
            .as_deref()
            != Some(owner)
        {
            return;
        }
        let Some(arg) = util::arg(node, 1).map(syntax::strip_parens) else {
            return;
        };
        if syntax::is_string_literal(arg) {
            Self::report(ctx, arg, arg);
        } else if let Some(lit) = constant_literal(ctx, arg) {
            Self::report(ctx, arg, lit);
        }
    }

    /// Reports `at` when the string literal `lit` names a standard charset.
    fn report<'t>(ctx: &Ctx<'t>, at: Node<'t>, lit: Node<'t>) {
        let Some(name) = syntax::string_literal_content(lit, ctx.source()) else {
            return;
        };
        if name.len() > 16 {
            return;
        }
        let Some(constant) = standard_constant(name) else {
            return;
        };
        ctx.emit(
            at,
            format!("Use `StandardCharsets.{constant}` instead of the charset name \"{name}\""),
        );
    }
}

impl<'t> JavaRule<'t> for StandardCharsetLookup {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation", "object_creation_expression"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["forName", "getBytes", "encode", "decode"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if kind(node) == "method_invocation" {
            let Some(name) =
                util::call_named(ctx, node, &["forName", "getBytes", "encode", "decode"])
            else {
                return;
            };
            if matches!(name, "encode" | "decode") {
                return Self::url_coding(ctx, node, name);
            }
            if util::arg_count(node) != 1 {
                return;
            }
            let Some(lit) = util::arg(node, 0) else {
                return;
            };
            if !syntax::is_string_literal(lit) {
                return;
            }
            let ok = if name == "forName" {
                ctx.is_call_to(node, "java.nio.charset.Charset", "forName")
            } else {
                util::fld(node, F.object)
                    .and_then(|o| ctx.type_of(o))
                    .is_some_and(|t| t.is_string())
            };
            if ok {
                Self::report(ctx, lit, lit);
            }
            return;
        }
        // `new String(bytes, "UTF-8")`, `new InputStreamReader(in, "UTF-8")`, …
        let Some(ty) = util::fld(node, F.ty) else {
            return;
        };
        let simple = ctx.text(ty).rsplit('.').next().unwrap_or("");
        if !matches!(
            simple,
            "String" | "InputStreamReader" | "OutputStreamWriter"
        ) {
            return;
        }
        let n = util::arg_count(node);
        if !(2..=4).contains(&n) {
            return;
        }
        let Some(lit) = util::args(node).last() else {
            return;
        };
        if !syntax::is_string_literal(lit) || util::anonymous_body(node).is_some() {
            return;
        }
        let fqn = util::type_fqn(ctx, ty);
        let ok = match fqn.as_deref() {
            Some("java.lang.String") => n == 2 || n == 4,
            Some("java.io.InputStreamReader" | "java.io.OutputStreamWriter") => n == 2,
            _ => false,
        };
        if ok {
            Self::report(ctx, lit, lit);
        }
    }
}
