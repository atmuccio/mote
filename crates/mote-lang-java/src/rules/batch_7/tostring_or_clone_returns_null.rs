//! `java/tostring-or-clone-returns-null` — `toString()` or `clone()` returning `null`.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["null-safety", "api-misuse"],
    remediation_minutes: 5,
    description: r#"Detects `return null;` in an overriding `toString()` or `clone()`.

**Why it matters.** Callers rely on these methods never returning `null`: string
concatenation, loggers, `String.valueOf`, debuggers and collection `toString()` call
`toString()` freely, and code that clones an object uses the copy immediately. A `null` from
either one surfaces as a `NullPointerException` far from its cause, or as the text `null` in
logs and user-facing messages.

**How to fix.** Return a meaningful string (possibly empty) from `toString()`; from `clone()`
return a copy, or throw `CloneNotSupportedException` / `AssertionError` when cloning is
impossible.

```java
// Noncompliant
@Override
public String toString() {
    if (name == null) {
        return null;
    }
    return name;
}

// Compliant
@Override
public String toString() {
    return name == null ? "" : name;
}
```

Only a literal `return null;` (directly, or as a branch of a conditional expression) in a
zero-parameter `toString()` or `clone()` is reported; lambdas and inner classes in the method
are skipped."#,
    ..RuleMeta::new(
        "java/tostring-or-clone-returns-null",
        "java",
        "toString() or clone() returns null",
    )
};

pub struct TostringOrCloneReturnsNull;

fn yields_null(node: Node<'_>) -> bool {
    let node = syntax::strip_parens(node);
    match kind(node) {
        "null_literal" => true,
        "ternary_expression" => {
            node.field(Field::Consequence).is_some_and(yields_null)
                || node.field(Field::Alternative).is_some_and(yields_null)
        }
        _ => false,
    }
}

impl<'t> JavaRule<'t> for TostringOrCloneReturnsNull {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_declaration"]
    }

    fn method_names(&self) -> &'static [&'static str] {
        &["toString", "clone"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name_node) = node.field(Field::Name) else {
            return;
        };
        let name = ctx.text(name_node);
        if !matches!(name, "toString" | "clone") || syntax::parameters(node).next().is_some() {
            return;
        }
        if syntax::has_modifier(node, "static") {
            return;
        }
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        if !ctx.text(body).contains("null") {
            return;
        }
        let ret = util::find_in_scope(body, |n| {
            kind(n) == "return_statement" && syntax::first_named_child(n).is_some_and(yields_null)
        });
        if let Some(ret) = ret {
            ctx.emit(
                ret,
                format!("Don't return null from `{name}()`: callers use its result without a null check"),
            );
        }
    }
}
