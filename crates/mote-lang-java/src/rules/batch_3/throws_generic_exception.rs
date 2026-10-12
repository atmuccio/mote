//! `java/throws-generic-exception` — `throws Exception` / `throws Throwable` on a method.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["error-handling", "api-design"],
    remediation_minutes: 10,
    description: r#"Detects methods and constructors that declare `throws Exception` or
`throws Throwable`.

**Why it matters.** A generic `throws` clause tells callers nothing about what can go wrong,
and forces every one of them to catch (or re-declare) `Exception`, which in turn swallows
unrelated runtime exceptions. The real failure modes of the method disappear from its
signature.

**How to fix.** Declare the specific checked exceptions the method throws, or wrap them in a
meaningful domain exception.

```java
// Noncompliant
public Report load(Path file) throws Exception { … }

// Compliant
public Report load(Path file) throws IOException, ReportFormatException { … }
```

Not reported: `main`, methods annotated `@Override` (the signature is dictated by the
supertype), methods of anonymous classes, well-known callbacks whose supertype declares the
generic exception (`call()`, `close()`, `finalize()`, `invoke(proxy, method, args)`), test
lifecycle methods, and test code."#,
    ..RuleMeta::new(
        "java/throws-generic-exception",
        "java",
        "Method throws a generic exception",
    )
};

/// Test lifecycle annotations: their signature is conventional, not an API.
const LIFECYCLE: &[&str] = &[
    "Before",
    "After",
    "BeforeClass",
    "AfterClass",
    "BeforeEach",
    "AfterEach",
    "BeforeAll",
    "AfterAll",
    "BeforeMethod",
    "AfterMethod",
    "Test",
];

pub struct ThrowsGenericException;

impl<'t> JavaRule<'t> for ThrowsGenericException {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["throws"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if ctx.is_test() {
            return;
        }
        // Cheap test first: is any listed type named Exception/Throwable?
        if !named_children(node)
            .any(|t| matches!(util::type_simple_name(t, ctx), "Exception" | "Throwable"))
        {
            return;
        }
        let Some(decl) = ctx.parent(node) else {
            return;
        };
        if !matches!(kind(decl), "method_declaration" | "constructor_declaration") {
            return;
        }
        if kind(decl) == "method_declaration" && is_exempt_method(decl, ctx) {
            return;
        }
        for t in named_children(node) {
            let simple = util::type_simple_name(t, ctx);
            if !matches!(simple, "Exception" | "Throwable") {
                continue;
            }
            let Some(fqn) = util::type_fqn(ctx, t) else {
                continue;
            };
            if matches!(fqn.as_str(), "java.lang.Exception" | "java.lang.Throwable") {
                ctx.emit(
                    t,
                    format!(
                        "Declare the specific exceptions this method throws instead of `{simple}`"
                    ),
                );
            }
        }
    }
}

fn is_exempt_method<'t>(decl: Node<'t>, ctx: &Ctx<'t>) -> bool {
    let src = ctx.source();
    let name = syntax::name_of(decl, src);
    let arity = syntax::parameters(decl).count();
    if name == "main"
        || matches!(
            (name, arity),
            ("call", 0) | ("close", 0) | ("finalize", 0) | ("invoke", 3)
        )
    {
        return true;
    }
    if syntax::annotations(decl).any(|a| {
        let n = syntax::annotation_name(a, src);
        n == "Override" || LIFECYCLE.contains(&n)
    }) {
        return true;
    }
    // Methods of anonymous classes implement or override a supertype method.
    matches!(
        ctx.enclosing_class(decl).map(kind),
        Some("object_creation_expression" | "enum_constant")
    )
}
