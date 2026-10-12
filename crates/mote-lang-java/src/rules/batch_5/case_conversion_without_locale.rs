//! `java/case-conversion-without-locale` — `s.toUpperCase()` depends on the default locale.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["i18n", "locale"],
    remediation_minutes: 2,
    description: r#"Detects `String.toUpperCase()` and `String.toLowerCase()` called without a
`Locale`.

**Why it matters.** Without an argument the conversion uses the JVM's default locale, so the
result depends on where the program runs. The classic surprise is Turkish: `"TITLE".toLowerCase()`
gives `"tıtle"` (dotless ı), which then fails to match keys, file names, SQL keywords, or
protocol tokens. Such bugs only show up on machines configured with that locale.

**How to fix.** Pass `Locale.ROOT` (or `Locale.ENGLISH`) for machine-readable text; pass the
user's locale explicitly when the text is meant for display.

```java
// Noncompliant
String key = name.toLowerCase();

// Compliant
String key = name.toLowerCase(Locale.ROOT);
String title = heading.toUpperCase(userLocale);
```

Only receivers known to be a `String` are checked: a value whose type the resolver knows, the
result of any `toString()`, an enum's own `name()`, and the parameter of a lambda passed to a
stream operation over strings (`names.stream().filter(n -> n.toLowerCase()…)`), and the file
name a `FilenameFilter` lambda receives (`dir.list((d, name) -> name.toLowerCase()…)`, also
for `File.listFiles` and `FileDialog.setFilenameFilter`). For
comparisons, `equalsIgnoreCase` avoids the conversion altogether."#,
    ..RuleMeta::new(
        "java/case-conversion-without-locale",
        "java",
        "Case conversion without a Locale",
    )
};

pub struct CaseConversionWithoutLocale;

/// Stream and collection operations whose single-parameter lambda receives the elements.
const ELEMENT_LAMBDA_METHODS: &[&str] = &[
    "filter",
    "map",
    "flatMap",
    "forEach",
    "forEachOrdered",
    "anyMatch",
    "allMatch",
    "noneMatch",
    "peek",
    "takeWhile",
    "dropWhile",
    "mapToInt",
    "mapToLong",
    "mapToDouble",
    "removeIf",
];

/// `x.toString()`, `super.toString()`, or a bare/`this.` `name()` inside an enum: a `String`
/// even when the receiver's type is unknown.
fn is_string_call<'t>(ctx: &Ctx<'t>, obj: Node<'t>) -> bool {
    let obj = syntax::strip_parens(obj);
    if kind(obj) != "method_invocation" || util::arg_count(obj) != 0 {
        return false;
    }
    match util::call_name(ctx, obj) {
        "toString" => true,
        "name" => {
            util::fld(obj, F.object).is_none_or(|o| kind(o) == "this")
                && ctx
                    .enclosing_class(obj)
                    .is_some_and(|c| matches!(kind(c), "enum_declaration" | "enum_constant"))
        }
        _ => false,
    }
}

/// A lambda parameter whose lambda is the single argument of an element operation
/// (`filter`, `map`, `forEach`, …) on a `Stream<String>` or a `Collection<String>`.
fn is_string_element_parameter<'t>(ctx: &Ctx<'t>, obj: Node<'t>) -> bool {
    if kind(obj) != "identifier" {
        return false;
    }
    let Some(d) = ctx.declaration_of(obj) else {
        return false;
    };
    if d.kind != VarKind::LambdaParameter {
        return false;
    }
    // The declaration is the lambda itself (or a parameter inside its parameter list).
    let mut lambda = Some(d.decl);
    while let Some(l) = lambda.filter(|l| kind(*l) != "lambda_expression") {
        if !matches!(
            kind(l),
            "inferred_parameters" | "formal_parameters" | "formal_parameter"
        ) {
            return false;
        }
        lambda = ctx.parent(l);
    }
    let Some(lambda) = lambda else {
        return false;
    };
    let single_param = util::fld(lambda, F.parameters)
        .is_some_and(|p| kind(p) == "identifier" || syntax::named_children(p).count() == 1);
    if !single_param {
        return false;
    }
    let Some(args) = ctx.parent(lambda).filter(|a| kind(*a) == "argument_list") else {
        return false;
    };
    let Some(call) = ctx.parent(args).filter(|c| kind(*c) == "method_invocation") else {
        return false;
    };
    if util::arg_count(call) != 1 || !ELEMENT_LAMBDA_METHODS.contains(&util::call_name(ctx, call)) {
        return false;
    }
    let Some(t) = util::fld(call, F.object).and_then(|r| ctx.type_of(r)) else {
        return false;
    };
    let element = ctx
        .as_supertype(&t, "java.util.stream.Stream")
        .or_else(|| ctx.as_supertype(&t, "java.lang.Iterable"))
        .and_then(|s| s.type_arg(0).cloned());
    element.is_some_and(|e| e.is_string())
}

/// JDK methods whose only argument is a `java.io.FilenameFilter`: a two-parameter lambda passed
/// to them receives the file name (a `String`) as its second parameter.
const FILENAME_FILTER_METHODS: &[(&str, &str)] = &[
    ("java.io.File", "list"),
    ("java.io.File", "listFiles"),
    ("java.awt.FileDialog", "setFilenameFilter"),
];

/// The lambda declaring the lambda parameter `d` (the declaration is the lambda itself or a
/// parameter inside its parameter list).
fn declaring_lambda<'t>(ctx: &Ctx<'t>, decl: Node<'t>) -> Option<Node<'t>> {
    let mut l = decl;
    while kind(l) != "lambda_expression" {
        if !matches!(
            kind(l),
            "inferred_parameters" | "formal_parameters" | "formal_parameter" | "identifier"
        ) {
            return None;
        }
        l = ctx.parent(l)?;
    }
    Some(l)
}

/// The second parameter of a two-parameter lambda that is the single argument of
/// `File.list(..)`, `File.listFiles(..)` or `FileDialog.setFilenameFilter(..)`: a
/// `FilenameFilter`'s file name (`(dir, name) -> name.toLowerCase().endsWith(".pdf")`).
fn is_filename_filter_name<'t>(ctx: &Ctx<'t>, obj: Node<'t>) -> bool {
    if kind(obj) != "identifier" {
        return false;
    }
    let Some(d) = ctx.declaration_of(obj) else {
        return false;
    };
    if d.kind != VarKind::LambdaParameter {
        return false;
    }
    let Some(lambda) = declaring_lambda(ctx, d.decl) else {
        return false;
    };
    let Some(params) = util::fld(lambda, F.parameters) else {
        return false;
    };
    // `(dir, name)` or `(File dir, String name)`: exactly two, and `obj` is the second.
    let names: Vec<&str> = syntax::named_children(params)
        .filter_map(|p| match kind(p) {
            "identifier" => Some(ctx.text(p)),
            "formal_parameter" => p.field(Field::Name).map(|n| ctx.text(n)),
            _ => None,
        })
        .collect();
    if names.len() != 2 || names[1] != ctx.text(obj) {
        return false;
    }
    let Some(args) = ctx.parent(lambda).filter(|a| kind(*a) == "argument_list") else {
        return false;
    };
    let Some(call) = ctx.parent(args).filter(|c| kind(*c) == "method_invocation") else {
        return false;
    };
    if util::arg_count(call) != 1 {
        return false;
    }
    let method = util::call_name(ctx, call);
    let Some(recv) = util::fld(call, F.object) else {
        return false;
    };
    // The receiver's type: resolved, or (for a library type outside the resolver's table, such
    // as `FileDialog`) the declared type of the variable, through the imports.
    let owner = ctx
        .type_of(recv)
        .and_then(|t| t.fqn().map(str::to_string))
        .or_else(|| {
            let r = syntax::strip_parens(recv);
            (kind(r) == "identifier")
                .then(|| ctx.declaration_of(r))
                .flatten()
                .and_then(|v| v.type_node)
                .and_then(|t| util::type_fqn(ctx, t))
        });
    owner.is_some_and(|o| {
        FILENAME_FILTER_METHODS
            .iter()
            .any(|(c, m)| *c == o && *m == method)
    })
}

impl<'t> JavaRule<'t> for CaseConversionWithoutLocale {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["toUpperCase", "toLowerCase"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if util::call_named(ctx, node, &["toUpperCase", "toLowerCase"]).is_none() {
            return;
        }
        let Some(name) = util::fld(node, F.name) else {
            return;
        };
        let text = ctx.text(name);
        if text != "toUpperCase" && text != "toLowerCase" {
            return;
        }
        if util::arg_count(node) != 0 {
            return;
        }
        let Some(obj) = util::fld(node, F.object) else {
            return;
        };
        let is_string = match ctx.type_of(obj) {
            Some(t) => t.is_string(),
            None => {
                is_string_call(ctx, obj)
                    || is_string_element_parameter(ctx, obj)
                    || is_filename_filter_name(ctx, obj)
            }
        };
        if !is_string {
            return;
        }
        ctx.emit(
            name,
            format!("Pass a `Locale` to `{text}()`, for example `Locale.ROOT`"),
        );
    }
}
