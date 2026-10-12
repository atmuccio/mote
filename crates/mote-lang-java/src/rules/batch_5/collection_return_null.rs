//! `java/collection-return-null` — `return null;` from a method returning a collection or
//! an array.

use super::util::{self, Mods, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["null", "collections"],
    remediation_minutes: 5,
    description: r#"Detects `return null;` in a method declared to return a collection
(`Collection`, `List`, `Set`, `Queue`, … or any implementation), a `Map`, or an array.

**Why it matters.** Callers of such methods naturally loop over the result or ask for its
size. A `null` breaks that expectation: every caller must remember a special check, and the
one that forgets fails with a `NullPointerException` far from the cause. An empty result says
"nothing" just as clearly and needs no special handling.

**How to fix.** Return an empty collection or array.

```java
// Noncompliant
List<Order> findOrders(Customer c) {
    if (c == null) {
        return null;
    }
    return repository.ordersOf(c);
}

// Compliant
List<Order> findOrders(Customer c) {
    if (c == null) {
        return List.of();          // or Collections.emptyList(), new Order[0], …
    }
    return repository.ordersOf(c);
}
```

Only the literal `null` is reported — returned directly or as one arm of a conditional
expression (`return found ? list : null;`) — only in methods (not lambdas), and only when the
declared return type is known to be an array, a `java.util.Collection`, or a `java.util.Map`
(also JDK classes that are maps by inheritance, such as `java.util.jar.Attributes`; a
`java.security.Provider`, a map of service properties, is not reported).
Methods annotated `@Nullable` (or `@CheckForNull`) document the `null` deliberately and are
not reported, and so are methods whose Javadoc `@return` mentions `null` (`@return the
matched groups, or null if there is no match`): there `null` is a documented result of its
own, often distinct from an empty one. Neither are `byte[]` and `char[]` results, which
usually carry data where `null` means "no value" (a SQL `NULL`, as in `ResultSet.getBytes`), nor test code, where
stubs and mocks return `null` on purpose."#,
    ..RuleMeta::new(
        "java/collection-return-null",
        "java",
        "Null returned instead of an empty collection or array",
    )
};

pub struct CollectionReturnNull;

/// The `null` literal a returned expression can evaluate to: the expression itself, or an arm of
/// a (nested) conditional expression.
fn returned_null(value: Node<'_>, depth: u32) -> Option<Node<'_>> {
    let v = syntax::strip_parens(value);
    if syntax::is_null_literal(v) {
        return Some(v);
    }
    if kind(v) != "ternary_expression" || depth > 4 {
        return None;
    }
    [F.consequence, F.alternative]
        .into_iter()
        .filter_map(|f| util::fld(v, f))
        .find_map(|arm| returned_null(arm, depth + 1))
}

/// Whether the method's Javadoc has an `@return` clause mentioning `null`.
fn documents_null_return<'t>(ctx: &Ctx<'t>, method: Node<'t>) -> bool {
    let Some(doc) = ctx.prev_named_sibling(method) else {
        return false;
    };
    let text = ctx.text(doc);
    if kind(doc) != "block_comment" || !text.starts_with("/**") {
        return false;
    }
    let Some(at) = text.find("@return") else {
        return false;
    };
    let clause = &text[at + "@return".len()..];
    // Up to the next block tag (`@param`, `@throws`, … at the start of a line).
    let end = clause
        .match_indices('@')
        .find(|(i, _)| {
            clause[..*i]
                .rsplit('\n')
                .next()
                .is_some_and(|line| line.trim_start_matches([' ', '\t', '*']).is_empty())
        })
        .map_or(clause.len(), |(i, _)| i);
    let clause = &clause[..end];
    let b = clause.as_bytes();
    clause.match_indices("null").any(|(i, _)| {
        let before = i.checked_sub(1).map(|j| b[j]);
        let after = b.get(i + 4).copied();
        !before.is_some_and(|c| c.is_ascii_alphanumeric() || c == b'_')
            && !after.is_some_and(|c| c.is_ascii_alphanumeric() || c == b'_')
    })
}

impl<'t> JavaRule<'t> for CollectionReturnNull {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["return_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if ctx.is_test() {
            return;
        }
        let Some(returned) = util::first_named(node) else {
            return;
        };
        let Some(value) = returned_null(returned, 0) else {
            return;
        };
        let Some(func) = ctx.enclosing_function(node) else {
            return;
        };
        if kind(func) != "method_declaration" {
            return;
        }
        let Some(ret) = util::fld(func, F.ty) else {
            return;
        };
        let mods = Mods::of(func);
        let src = ctx.source();
        if mods.has_annotation(src, "Nullable") || mods.has_annotation(src, "CheckForNull") {
            return;
        }
        // `@Nullable` may also sit on the return type itself (type annotations).
        if ctx.text(ret).contains("Nullable") {
            return;
        }
        if documents_null_return(ctx, func) {
            return;
        }
        let what = if kind(ret) == "array_type" || util::fld(func, F.dimensions).is_some() {
            // `byte[]` / `char[]` are data values: `null` usually means "no value" (SQL NULL),
            // which an empty array would silently change.
            let element = if kind(ret) == "array_type" {
                util::fld(ret, F.element).map_or("", |e| ctx.text(e))
            } else {
                ctx.text(ret)
            };
            if element == "byte" || element == "char" {
                return;
            }
            "an empty array"
        } else {
            let Some(t) = ctx.resolve_type(ret) else {
                return;
            };
            // A security provider is a map of service properties, not a result collection.
            if ctx.is_subtype_of(&t, "java.security.Provider") {
                return;
            }
            if ctx.is_subtype_of(&t, "java.util.Map") {
                "an empty map"
            } else if ctx.is_subtype_of(&t, "java.util.Collection") {
                "an empty collection"
            } else {
                return;
            }
        };
        ctx.emit(value, format!("Return {what} instead of `null`"));
    }
}
