//! `java/redundant-parentheses` — parentheses around a returned value, around a single operand,
//! or doubled.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["conventions", "clutter"],
    remediation_minutes: 1,
    description: r#"Detects parentheses that group nothing:

* around the whole value of a `return`, `throw` or `yield` (`return (count);`,
  `return (a + b);`);
* around a single operand: a name, a literal, a field access, a call, an object creation or an
  array element (`(name).length()`, `total += (price);`, `values[(i)]`);
* doubled parentheses (`((a + b))`, `if ((ready))`).

**Why it matters.** Parentheses tell the reader that grouping matters here. Where there is
nothing to group, the reader looks for the precedence problem they solve and finds none, and
`return (x);` makes `return` look like a function call. Java formatters keep such parentheses,
so they stay until someone removes them.

**How to fix.** Remove the parentheses.

```java
// Noncompliant
return (result);
String label = (name).trim();
if ((enabled)) { … }

// Compliant
return result;
String label = name.trim();
if (enabled) { … }
```

Not reported: parentheses around an operation inside another expression, which may clarify
precedence even where it is not needed (`a + (b * c)`, `(a && b) || c`); a returned
conditional expression or assignment (`return (ok ? a : b);`), which some conventions
parenthesize on purpose; parentheses around a whole condition assigned to a variable
(`boolean empty = (size == 0);`); and parentheses that contain a comment. The rule is off by
default: it is part of the `conventions` profile."#,
    ..RuleMeta::new(
        "java/redundant-parentheses",
        "java",
        "Parentheses that group nothing",
    )
};

#[derive(Default)]
pub struct RedundantParentheses {
    /// [`in_misparsed_case_label`] of the parenthesized expressions checked so far, by node id:
    /// nested parentheses reuse their outer pair's answer instead of each walking every ancestor
    /// (quadratic in the nesting depth).
    in_label: FxHashMap<usize, bool>,
}

/// Statements whose syntax requires the parentheses around their condition or lock.
fn is_syntactic(parent: Node<'_>) -> bool {
    matches!(
        kind(parent),
        "if_statement"
            | "while_statement"
            | "do_statement"
            | "switch_expression"
            | "synchronized_statement"
    )
}

/// A single operand: nothing inside it can bind differently with or without parentheses.
fn is_single_operand(node: Node<'_>) -> bool {
    matches!(
        kind(node),
        "identifier"
            | "this"
            | "field_access"
            | "method_invocation"
            | "object_creation_expression"
            | "array_access"
            | "class_literal"
            | "string_literal"
            | "character_literal"
            | "decimal_integer_literal"
            | "hex_integer_literal"
            | "octal_integer_literal"
            | "binary_integer_literal"
            | "decimal_floating_point_literal"
            | "hex_floating_point_literal"
            | "true"
            | "false"
            | "null_literal"
    )
}

/// Whether `node` sits in a `case` label that the parser read as a lambda: in
/// `case A -> f = (x) -> g(x);` the label becomes the "lambda" `A -> f = (x)`, and `(x)`, the
/// real lambda's parameter list, looks like a parenthesized name. A label never holds a lambda.
///
/// `memo` holds the answers for parenthesized expressions already checked: the walk stops at the
/// nearest of them (a parenthesized expression is neither a label nor a boundary, so the walk
/// from there on is the one that produced its answer).
fn in_misparsed_case_label<'t>(
    ctx: &Ctx<'t>,
    node: Node<'t>,
    memo: &mut FxHashMap<usize, bool>,
) -> bool {
    let mut answer = false;
    for a in ctx.ancestors(node) {
        if let Some(&known) = memo.get(&a.id()) {
            answer = known;
            break;
        }
        match kind(a) {
            "switch_label" => {
                answer = syntax::named_children(a).any(|c| kind(c) == "lambda_expression");
                break;
            }
            "block" | "class_body" | "switch_block" | "program" => break,
            _ => {}
        }
    }
    memo.insert(node.id(), answer);
    answer
}

impl<'t> JavaRule<'t> for RedundantParentheses {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["parenthesized_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(parent) = ctx.parent(node) else {
            return;
        };
        if is_syntactic(parent) {
            return;
        }
        // Exactly one named child, and no comment inside.
        let mut inner = None;
        for c in syntax::children(node) {
            if !c.is_named() {
                continue;
            }
            if syntax::is_comment(c) || inner.is_some() {
                return;
            }
            inner = Some(c);
        }
        let Some(inner) = inner else {
            return;
        };
        if in_misparsed_case_label(ctx, node, &mut self.in_label) {
            return;
        }
        let inner_kind = kind(inner);
        let message = if inner_kind == "parenthesized_expression" {
            "Remove one pair of these doubled parentheses"
        } else if kind(parent) == "parenthesized_expression" {
            // Doubled: reported once, on the outer pair, unless the outer pair belongs to an
            // `if` / `while` / … condition.
            if !ctx.parent(parent).is_some_and(is_syntactic) {
                return;
            }
            "Remove one pair of these doubled parentheses"
        } else if matches!(
            kind(parent),
            "return_statement" | "throw_statement" | "yield_statement"
        ) {
            if matches!(
                inner_kind,
                "ternary_expression" | "assignment_expression" | "lambda_expression"
            ) {
                return;
            }
            match kind(parent) {
                "return_statement" => "Remove the parentheses around the returned value",
                "throw_statement" => "Remove the parentheses around the thrown exception",
                _ => "Remove the parentheses around the yielded value",
            }
        } else if is_single_operand(inner) {
            "Remove the parentheses around this single operand"
        } else {
            return;
        };
        ctx.emit(node, message);
    }
}
