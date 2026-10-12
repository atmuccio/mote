//! `java/nested-try` — a try-catch statement directly in the body of another try-catch.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["exceptions", "structure"],
    remediation_minutes: 10,
    description: r#"Detects a `try` statement that is a statement of the body of another `try`
statement when both have `catch` clauses for overlapping exception types: the same type, one
a subtype of the other, or a catch-all (`Exception`, `Throwable`).

**Why it matters.** Code in the inner body is covered by two sets of handlers that could both
take the same failure. To know what happens when it throws, a reader has to match the
exception against the inner catches, then the outer ones, and keep in mind that the inner
handlers can themselves throw into the outer ones. The nesting also usually means one method
is handling failures of two different operations.

**How to fix.** Extract the inner `try` into a method named after what it does, or merge the
handlers into one `try` when they handle the same failure.

```java
// Noncompliant
try {
    Config config = load(path);
    try {
        connect(config);
    } catch (IOException e) {
        useOfflineMode();
    }
} catch (IOException e) {
    reportUnreadable(path, e);
}

// Compliant
try {
    Config config = load(path);
    connectOrGoOffline(config);
} catch (IOException e) {
    reportUnreadable(path, e);
}
```

Not reported: handlers for unrelated exception types (each failure has one obvious handler),
an inner or outer `try` without `catch` clauses (`try`-`finally` and
try-with-resources used for cleanup), and tries nested in a `catch` or `finally` block, in a
loop or in a conditional inside the outer body (cleanup that can fail and per-item handling
are common and deliberate)."#,
    ..RuleMeta::new("java/nested-try", "java", "Nested try-catch")
};

pub struct NestedTry;

/// The exception types of a try statement's catch clauses.
fn catch_types(t: Node<'_>) -> Vec<Node<'_>> {
    let mut out = Vec::new();
    for c in named_children(t) {
        if kind(c) != "catch_clause" {
            continue;
        }
        let Some(p) = syntax::child_of_kind(c, "catch_formal_parameter") else {
            continue;
        };
        let Some(ct) = syntax::child_of_kind(p, "catch_type") else {
            continue;
        };
        out.extend(named_children(ct));
    }
    out
}

/// Whether one catch type can catch what the other catches: the same name, a supertype
/// relation the resolver proves, or a catch-all (`Throwable`, or `Exception` against a type
/// named `…Exception`).
fn overlap<'t>(ctx: &Ctx<'t>, a: Node<'t>, b: Node<'t>) -> bool {
    let (sa, sb) = (
        util::last_segment(ctx.text(a)),
        util::last_segment(ctx.text(b)),
    );
    if sa == sb {
        return true;
    }
    let catch_all = |s: &str, other: &str| {
        s == "Throwable" || (s == "Exception" && other.ends_with("Exception"))
    };
    if catch_all(sa, sb) || catch_all(sb, sa) {
        return true;
    }
    let (Some(ta), Some(tb)) = (ctx.resolve_type(a), ctx.resolve_type(b)) else {
        return false;
    };
    let (Some(fa), Some(fb)) = (ta.fqn(), tb.fqn()) else {
        return false;
    };
    ctx.is_subtype_of(&ta, fb) || ctx.is_subtype_of(&tb, fa)
}

impl<'t> JavaRule<'t> for NestedTry {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["try_statement", "try_with_resources_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let mut anc = ctx.ancestors(node);
        let (Some(block), Some(outer)) = (anc.next(), anc.next()) else {
            return;
        };
        if kind(block) != "block"
            || !matches!(
                kind(outer),
                "try_statement" | "try_with_resources_statement"
            )
            || outer.field(Field::Body).map(|b| b.id()) != Some(block.id())
        {
            return;
        }
        let inner_types = catch_types(node);
        if inner_types.is_empty() {
            return;
        }
        let outer_types = catch_types(outer);
        let Some((i, o)) = inner_types.iter().find_map(|&i| {
            outer_types
                .iter()
                .find(|&&o| overlap(ctx, i, o))
                .map(|&o| (i, o))
        }) else {
            return;
        };
        let (it, ot) = (ctx.text(i), ctx.text(o));
        let msg = if it == ot {
            format!("Move this try-catch into its own method: it and the enclosing try both catch `{it}`")
        } else {
            format!("Move this try-catch into its own method: its `{it}` handler overlaps the enclosing try's `{ot}` handler")
        };
        let kw = node.child(0).unwrap_or(node);
        ctx.emit(kw, msg).secondary(o, "enclosing handler");
    }
}
