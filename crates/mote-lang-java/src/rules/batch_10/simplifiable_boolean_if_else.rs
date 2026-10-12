//! `java/simplifiable-boolean-if-else` — `if (c) b.setEnabled(true); else b.setEnabled(false);`.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["readability", "redundant"],
    remediation_minutes: 2,
    description: r#"Detects an `if`/`else` whose two branches make the same call, or the same
assignment, and differ only in a `true` / `false` literal: `if (c) button.setEnabled(true);
else button.setEnabled(false);`.

**Why it matters.** The condition already is the boolean being passed. Spelling out both
values doubles the code, hides that the two branches are really one statement, and invites
the branches to drift apart when one of them is edited later.

**How to fix.** Pass (or assign) the condition itself, negated with `!` when the `true` sits in
the `else` branch.

```java
// Noncompliant
if (rows.isEmpty()) {
    exportButton.setEnabled(false);
} else {
    exportButton.setEnabled(true);
}
if (limit > 0) {
    limited = true;
} else {
    limited = false;
}

// Compliant
exportButton.setEnabled(!rows.isEmpty());
limited = limit > 0;
```

Each branch must hold exactly one statement (no comment either). Only calls whose receiver and
other arguments are plain names, field reads, or literals are reported: moving the condition
into the argument list evaluates it after them, which only makes no difference when they have no
side effects. Not reported either: `else if` chains ending in another `if`, compound
assignments, and conditions of type `Boolean` (passing the object would hand a `null` on
instead of failing)."#,
    ..RuleMeta::new(
        "java/simplifiable-boolean-if-else",
        "java",
        "if/else that only differs in a boolean literal",
    )
};

pub struct SimplifiableBooleanIfElse;

/// The only statement of a branch: the statement itself, or a block holding exactly one
/// statement and no comment.
fn only_statement(branch: Node<'_>) -> Option<Node<'_>> {
    if kind(branch) != "block" {
        return Some(branch);
    }
    let mut it = syntax::children(branch).filter(|c| c.is_named());
    let first = it.next()?;
    (it.next().is_none() && !syntax::is_comment(first)).then_some(first)
}

/// The expression of an expression statement.
fn statement_expression(stmt: Node<'_>) -> Option<Node<'_>> {
    (kind(stmt) == "expression_statement")
        .then(|| syntax::first_named_child(stmt))
        .flatten()
}

fn bool_literal(n: Node<'_>) -> Option<bool> {
    match kind(syntax::strip_parens(n)) {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// Evaluating `n` has no side effect and does not depend on when it happens: a name, `this`, a
/// literal, a class literal, or a field read on one of those.
fn is_plain(n: Node<'_>, depth: u32) -> bool {
    if depth > 8 {
        return false;
    }
    let n = syntax::strip_parens(n);
    match kind(n) {
        "identifier"
        | "this"
        | "class_literal"
        | "string_literal"
        | "character_literal"
        | "decimal_integer_literal"
        | "hex_integer_literal"
        | "octal_integer_literal"
        | "binary_integer_literal"
        | "decimal_floating_point_literal"
        | "true"
        | "false"
        | "null_literal" => true,
        "field_access" => n
            .field(Field::Object)
            .is_some_and(|o| is_plain(o, depth + 1)),
        _ => false,
    }
}

/// Source text without whitespace, for comparing two expressions written the same way.
fn squeezed(ctx: &Ctx<'_>, n: Node<'_>) -> String {
    ctx.text(n).chars().filter(|c| !c.is_whitespace()).collect()
}

/// For two calls that are the same except for one `true`/`false` argument: the method name and
/// the value passed in the first call.
fn same_call_but_literal<'t>(ctx: &Ctx<'t>, a: Node<'t>, b: Node<'t>) -> Option<(&'t str, bool)> {
    let name = ctx.call_name(a)?;
    if ctx.call_name(b) != Some(name)
        || a.field(Field::TypeArguments).is_some()
        || b.field(Field::TypeArguments).is_some()
    {
        return None;
    }
    match (a.field(Field::Object), b.field(Field::Object)) {
        (None, None) => {}
        (Some(x), Some(y)) if is_plain(x, 0) && squeezed(ctx, x) == squeezed(ctx, y) => {}
        _ => return None,
    }
    if syntax::argument_count(a) != syntax::argument_count(b) {
        return None;
    }
    let mut differs: Option<bool> = None;
    for (x, y) in syntax::arguments(a).zip(syntax::arguments(b)) {
        match (bool_literal(x), bool_literal(y)) {
            (Some(p), Some(q)) if p != q => {
                if differs.is_some() {
                    return None;
                }
                differs = Some(p);
            }
            _ => {
                if !is_plain(x, 0) || squeezed(ctx, x) != squeezed(ctx, y) {
                    return None;
                }
            }
        }
    }
    differs.map(|first| (name, first))
}

/// For `x = true` / `x = false` on the same plain target: the target and the first value.
fn same_assignment_but_literal<'t>(
    ctx: &Ctx<'t>,
    a: Node<'t>,
    b: Node<'t>,
) -> Option<(Node<'t>, bool)> {
    if syntax::operator_str(a) != "=" || syntax::operator_str(b) != "=" {
        return None;
    }
    let (left_a, left_b) = (a.field(Field::Left)?, b.field(Field::Left)?);
    if !is_plain(left_a, 0) || squeezed(ctx, left_a) != squeezed(ctx, left_b) {
        return None;
    }
    let first = bool_literal(a.field(Field::Right)?)?;
    let second = bool_literal(b.field(Field::Right)?)?;
    (first != second).then_some((left_a, first))
}

impl<'t> JavaRule<'t> for SimplifiableBooleanIfElse {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["if_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let (Some(cons), Some(alt)) = (
            node.field(Field::Consequence),
            node.field(Field::Alternative),
        ) else {
            return;
        };
        if kind(alt) == "if_statement" {
            return;
        }
        let (Some(a), Some(b)) = (
            only_statement(cons).and_then(statement_expression),
            only_statement(alt).and_then(statement_expression),
        ) else {
            return;
        };
        let message = match (kind(a), kind(b)) {
            ("method_invocation", "method_invocation") => {
                let Some((name, first)) = same_call_but_literal(ctx, a, b) else {
                    return;
                };
                let negated = if first { "" } else { "negated " };
                format!(
                    "Pass the {negated}condition to `{name}(…)` instead of calling it with `true` \
                     and `false` in an if/else"
                )
            }
            ("assignment_expression", "assignment_expression") => {
                let Some((target, first)) = same_assignment_but_literal(ctx, a, b) else {
                    return;
                };
                let negated = if first { "" } else { "negated " };
                format!(
                    "Assign the {negated}condition to `{}` instead of assigning `true` and `false` \
                     in an if/else",
                    ctx.text(target)
                )
            }
            _ => return,
        };
        let Some(condition) = node.field(Field::Condition) else {
            return;
        };
        if ctx.type_of(condition).is_some_and(|t| t.is_boxed()) {
            return;
        }
        let at = syntax::children(node).next().unwrap_or(node);
        ctx.emit(at, message);
    }
}
