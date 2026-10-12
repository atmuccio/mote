//! `java/return-in-finally` — a syntactic rule using ancestors.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["error-handling", "control-flow"],
    remediation_minutes: 10,
    description: r#"Detects `return`, `throw`, `break`, and `continue` statements that leave a
`finally` block.

**Why it matters.** When control jumps out of a `finally` block, whatever the `try` block was
doing is silently discarded: an exception in flight disappears, and a value already being
returned is replaced. Failures become invisible and results surprising.

**How to fix.** Let the `finally` block complete normally; move the `return`/`throw` after the
`try` statement or into the `try`/`catch` blocks.

```java
// Noncompliant
try {
    return compute();
} finally {
    return -1;          // hides any exception from compute()
}

// Compliant
int result;
try {
    result = compute();
} finally {
    cleanup();
}
return result;
```

Jumps that stay inside the `finally` block (a `break` of a loop inside it, a `return` inside a
lambda or anonymous class declared there, a `throw` caught within it) are not reported."#,
    ..RuleMeta::new(
        "java/return-in-finally",
        "java",
        "Jump out of a finally block",
    )
};

pub struct ReturnInFinally;

impl<'t> JavaRule<'t> for ReturnInFinally {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "return_statement",
            "throw_statement",
            "break_statement",
            "continue_statement",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let k = kind(node);
        let label = if matches!(k, "break_statement" | "continue_statement") {
            named_children(node)
                .find(|c| kind(*c) == "identifier")
                .map(|l| ctx.text(l))
        } else {
            None
        };
        let mut prev = node;
        for a in ctx.ancestors(node) {
            match kind(a) {
                // Function and class boundaries: the jump can't leave an enclosing finally.
                "lambda_expression"
                | "class_body"
                | "method_declaration"
                | "constructor_declaration"
                | "compact_constructor_declaration"
                | "static_initializer"
                | "interface_body"
                | "enum_body" => {
                    return;
                }
                "for_statement" | "enhanced_for_statement" | "while_statement" | "do_statement" => {
                    if label.is_none() && matches!(k, "break_statement" | "continue_statement") {
                        return;
                    }
                }
                "switch_block" => {
                    if label.is_none() && k == "break_statement" {
                        return;
                    }
                }
                "labeled_statement" => {
                    let own = named_children(a)
                        .find(|c| kind(*c) == "identifier")
                        .map(|l| ctx.text(l));
                    if label.is_some() && own == label {
                        return;
                    }
                }
                "try_statement" | "try_with_resources_statement" => {
                    // A throw inside a try body that has catch clauses may be caught right there.
                    let in_body = a.field(Field::Body).is_some_and(|b| b.id() == prev.id());
                    let has_catch = named_children(a).any(|c| kind(c) == "catch_clause");
                    if k == "throw_statement" && in_body && has_catch {
                        return;
                    }
                }
                "finally_clause" => {
                    let keyword = syntax::children(node).next().unwrap_or(node);
                    let kw = ctx.text(keyword);
                    ctx.emit(
                        keyword,
                        format!("Remove this `{kw}` from the finally block; it discards any exception or result of the try block"),
                    );
                    return;
                }
                _ => {}
            }
            prev = a;
        }
    }
}
