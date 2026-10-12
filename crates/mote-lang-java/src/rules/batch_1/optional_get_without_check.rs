//! `java/optional-get-without-check` — `opt.get()` without a presence check.

use super::util::{self, Fld, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["optional", "null-safety"],
    remediation_minutes: 5,
    description: r#"Detects `get()` on an `Optional` variable (and `getAsInt()`, `getAsLong()`,
`getAsDouble()` on `OptionalInt`, `OptionalLong`, `OptionalDouble`) when the same method never
checked it earlier with `isPresent()`, `isEmpty()`, `ifPresent(..)`, or `ifPresentOrElse(..)`.

**Why it matters.** `get()` throws `NoSuchElementException` on an empty optional. Calling it
unchecked brings back exactly the failure `Optional` was meant to make explicit — just with a
different exception than `NullPointerException`.

**How to fix.** Handle the empty case: `orElse(..)`, `orElseGet(..)`, `map(..)`, `ifPresent(..)`,
or `orElseThrow(..)` with a meaningful exception. If emptiness is truly impossible, say so with
`orElseThrow()`, which reads as an assertion.

```java
// Noncompliant
Optional<User> user = repository.findById(id);
return user.get().getName();

// Compliant
Optional<User> user = repository.findById(id);
return user.map(User::getName).orElse("unknown");
```

Only local variables, parameters, and fields (`opt` or `this.opt`) are tracked, and a check
anywhere earlier in the same method (including code around a lambda or anonymous class, and
conditions such as `opt.isPresent() ? opt.get() : x`) counts. Optionals initialized with
`Optional.of(..)` and test code (where a failing `get()` just fails the test) are not
reported."#,
    ..RuleMeta::new(
        "java/optional-get-without-check",
        "java",
        "Optional.get() without a presence check",
    )
};

pub struct OptionalGetWithoutCheck;

/// The tracked name of a receiver: `opt` or `this.opt`.
fn receiver_name<'t>(ctx: &Ctx<'t>, recv: Node<'t>) -> Option<(Node<'t>, &'t str)> {
    match kind(recv) {
        "identifier" => Some((recv, ctx.text(recv))),
        "field_access" => {
            if recv.fld(F::Object).is_none_or(|o| kind(o) != "this") {
                return None;
            }
            let f = recv.fld(F::Field)?;
            Some((f, ctx.text(f)))
        }
        _ => None,
    }
}

/// Whether `text` contains `var.isPresent`, `var.isEmpty`, `var.ifPresent…` (also after
/// `this.`), with `var` not preceded by another identifier character.
fn has_presence_check(text: &str, var: &str) -> bool {
    if var.is_empty() {
        return false;
    }
    let bytes = text.as_bytes();
    let mut from = 0;
    while let Some(pos) = text.get(from..).and_then(|t| t.find(var)) {
        let start = from + pos;
        let end = start + var.len();
        from = end;
        let boundary = start == 0 || {
            let b = bytes[start - 1];
            !(b.is_ascii_alphanumeric() || b == b'_' || b == b'$' || b >= 0x80)
        };
        if !boundary {
            continue;
        }
        let rest = text.get(end..).unwrap_or("").trim_start();
        let Some(after_dot) = rest.strip_prefix('.') else {
            continue;
        };
        let after_dot = after_dot.trim_start();
        if after_dot.starts_with("isPresent")
            || after_dot.starts_with("isEmpty")
            || after_dot.starts_with("ifPresent")
        {
            return true;
        }
    }
    false
}

/// The outermost method, constructor, initializer, or field declaration around `node`: code in
/// a lambda or anonymous class runs after the checks written around it.
fn outermost_member<'t>(ctx: &Ctx<'t>, node: Node<'t>) -> Option<Node<'t>> {
    let mut found = None;
    let mut prev = node;
    for a in ctx.ancestors(node) {
        match kind(a) {
            "method_declaration"
            | "constructor_declaration"
            | "compact_constructor_declaration"
            | "static_initializer"
            | "field_declaration" => found = Some(a),
            "class_body" | "enum_body_declarations" if kind(prev) == "block" => found = Some(prev),
            // A named class ends the search: its members are separate scopes.
            "class_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "record_declaration" => break,
            _ => {}
        }
        prev = a;
    }
    found
}

impl<'t> JavaRule<'t> for OptionalGetWithoutCheck {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["get", "getAsInt", "getAsLong", "getAsDouble"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        // Text pre-filter (no tree access): a no-argument getter call.
        let text = ctx.text(node);
        let owner = if text.ends_with(".get()") {
            "java.util.Optional"
        } else if text.ends_with(".getAsInt()") {
            "java.util.OptionalInt"
        } else if text.ends_with(".getAsLong()") {
            "java.util.OptionalLong"
        } else if text.ends_with(".getAsDouble()") {
            "java.util.OptionalDouble"
        } else {
            return;
        };
        if ctx.is_test() {
            return;
        }
        let Some(recv) = node.fld(F::Object) else {
            return;
        };
        let recv = syntax::strip_parens(recv);
        let Some((ident, var)) = receiver_name(ctx, recv) else {
            return;
        };
        let Some(t) = ctx.type_of(recv) else { return };
        if !t.is_class(owner) {
            return;
        }
        // A check earlier in the same top-level member (lambdas and anonymous classes included),
        // found in the member's text.
        let Some(scope) = outermost_member(ctx, node) else {
            return;
        };
        let before = ctx
            .source()
            .get(scope.start_byte()..node.start_byte())
            .unwrap_or("");
        if has_presence_check(before, var) {
            return;
        }
        // `Optional.of(..)` can't be empty.
        if let Some(v) = ctx.declaration_of(ident).and_then(|d| d.value) {
            let v = syntax::strip_parens(v);
            if kind(v) == "method_invocation" && util::method_name(ctx, v) == "of" {
                return;
            }
        }
        let name = util::method_name(ctx, node);
        let at = node.fld(F::Name).unwrap_or(node);
        ctx.emit(
            at,
            format!("Check that `{var}` is present before calling {name}(), or use orElse/orElseThrow/map"),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::has_presence_check;

    #[test]
    fn finds_checks() {
        assert!(has_presence_check("if (opt.isPresent()) {", "opt"));
        assert!(has_presence_check("x; this.opt . isEmpty()", "opt"));
        assert!(has_presence_check("opt.ifPresent(v -> f(v));", "opt"));
        assert!(!has_presence_check("myopt.isPresent()", "opt"));
        assert!(!has_presence_check("opt.get(); other.isPresent()", "opt"));
        assert!(!has_presence_check("", "opt"));
        assert!(!has_presence_check("x.isPresent()", ""));
    }
}
