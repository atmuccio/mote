//! `java/url-in-hash-collection` — `java.net.URL` as a map key / set element, or URL.equals.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["performance", "network", "equality"],
    remediation_minutes: 10,
    description: r#"Detects `java.net.URL` used as the key type of a `Map` or the element type of a
`Set`, and direct calls to `URL.equals(..)` or `URL.hashCode()`.

**Why it matters.** `URL.equals` and `URL.hashCode` resolve the host name to an IP address.
Every lookup in a `HashMap<URL, …>` or `HashSet<URL>` may block on DNS, results change with
network conditions (two different virtual hosts on one IP are "equal"; offline, equal URLs
may not be), and the hash code of a key can differ between insertion and lookup.

**How to fix.** Use `java.net.URI` for identity and comparison (its `equals` is purely
textual), converting with `url.toURI()` / `uri.toURL()` at the boundaries, or key the
collection by `url.toExternalForm()`.

```java
// Noncompliant
Map<URL, Response> cache = new HashMap<>();

// Compliant
Map<URI, Response> cache = new HashMap<>();
```

Sorted maps and sets are not reported (`URL` is not `Comparable`, so they need a comparator
that never calls `equals`)."#,
    ..RuleMeta::new(
        "java/url-in-hash-collection",
        "java",
        "URL in a hash-based collection or compared with equals",
    )
};

const URL: &str = "java.net.URL";

/// Hash/equality-based collection types whose first type argument is the key or element.
const HASHED: &[&str] = &[
    "java.util.Map",
    "java.util.HashMap",
    "java.util.LinkedHashMap",
    "java.util.Hashtable",
    "java.util.WeakHashMap",
    "java.util.concurrent.ConcurrentMap",
    "java.util.concurrent.ConcurrentHashMap",
    "java.util.Set",
    "java.util.HashSet",
    "java.util.LinkedHashSet",
    "java.util.concurrent.CopyOnWriteArraySet",
];

#[derive(Default)]
pub struct UrlInHashCollection {
    url_in_file: util::Gate,
}

impl<'t> JavaRule<'t> for UrlInHashCollection {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["generic_type", "method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["equals", "hashCode"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if kind(node) == "method_invocation" {
            if !self
                .url_in_file
                .open_any(ctx, &["java.net.URL", "java.net.*"])
            {
                return;
            }
            let Some(name) = util::fld(node, F::Name) else {
                return;
            };
            let expected_args = match ctx.text(name) {
                "equals" => 1,
                "hashCode" => 0,
                _ => return,
            };
            if util::arg_count(node) != expected_args {
                return;
            }
            let Some(recv) = util::fld(node, F::Object) else {
                return;
            };
            if matches!(kind(recv), "this" | "super" | "string_literal") {
                return;
            }
            if ctx.type_of(recv).is_some_and(|t| t.is_class(URL)) {
                ctx.emit(
                    name,
                    format!("Avoid URL.{}(): it resolves the host name over the network; compare java.net.URI values instead", ctx.text(name)),
                );
            }
            return;
        }
        // generic_type: cheap text tests first.
        if !ctx.text(node).contains("URL")
            || !self
                .url_in_file
                .open_any(ctx, &["java.net.URL", "java.net.*"])
        {
            return;
        }
        // `Map<URL, X> m = new HashMap<URL, X>()` is reported once, on the declared type.
        if let Some(parent) = ctx.parent(node) {
            if kind(parent) == "object_creation_expression" {
                let declared_generic = ctx
                    .ancestors(node)
                    .nth(1)
                    .filter(|d| kind(*d) == "variable_declarator")
                    .and_then(|d| ctx.parent(d))
                    .and_then(|decl| util::fld(decl, F::Type))
                    .is_some_and(|t| kind(t) == "generic_type");
                if declared_generic {
                    return;
                }
            }
        }
        let Some(t) = ctx.resolve_type(node) else {
            return;
        };
        let Some(fqn) = t.fqn() else {
            return;
        };
        if !HASHED.contains(&fqn) {
            return;
        }
        if !t.type_arg(0).is_some_and(|a| a.is_class(URL)) {
            return;
        }
        let role = if fqn.ends_with("Set") {
            "element"
        } else {
            "key"
        };
        ctx.emit(
            node,
            format!("Use java.net.URI instead of URL as the {role} type: URL.equals() and hashCode() perform DNS lookups"),
        );
    }
}
