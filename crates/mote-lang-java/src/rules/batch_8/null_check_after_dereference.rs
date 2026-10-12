//! `java/null-check-after-dereference` — a variable is compared with `null` after the same
//! block has already dereferenced it.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["null", "suspicious"],
    remediation_minutes: 5,
    description: r#"Detects an `if` that compares a local variable or parameter with `null` after an
earlier statement of the same block has already dereferenced it (called a method on it, read
a field or an element, iterated over it, locked on it), with no assignment to the variable in
between.

**Why it matters.** Had the variable been `null`, the earlier dereference would have thrown a
`NullPointerException` and the check would never run. So one of the two is wrong: either the
value can be `null` and the dereference comes too early (the check was meant to protect it),
or it cannot and the check is dead code that misleads readers into thinking `null` is
possible here.

**How to fix.** Move the check before the first use, or remove it when the value can never be
`null`.

```java
// Noncompliant
String name = user.getName();
int length = name.length();
if (name != null) { … }          // too late: length() already failed on null

// Compliant
String name = user.getName();
if (name != null) {
    int length = name.length();
    …
}
```

Only the 16 statements before the check are examined, only locals and parameters are
considered (a field can be changed by the calls in between),
and only dereferences that always run: those inside lambdas, anonymous classes, the right
side of `&&` / `||`, a branch of `?:`, a loop body, a `try` block, or an `assert` are ignored."#,
    ..RuleMeta::new(
        "java/null-check-after-dereference",
        "java",
        "Null check after the value was dereferenced",
    )
};

pub struct NullCheckAfterDereference;

/// How many statements before the `if` are examined.
const MAX_STATEMENTS: usize = 16;

/// `x == null` / `x != null` comparisons in a condition (not inside lambdas), with the
/// compared identifier.
fn null_comparisons<'t>(cond: Node<'t>, depth: u32, out: &mut Vec<(Node<'t>, Node<'t>)>) {
    if depth > 32 || out.len() > 4 {
        return;
    }
    match kind(cond) {
        "lambda_expression" | "class_body" | "switch_expression" => {}
        "binary_expression" => {
            let op = syntax::operator_str(cond);
            if op == "==" || op == "!=" {
                let (Some(l), Some(r)) = (cond.field(Field::Left), cond.field(Field::Right)) else {
                    return;
                };
                let (l, r) = (syntax::strip_parens(l), syntax::strip_parens(r));
                if kind(r) == "null_literal" && kind(l) == "identifier" {
                    out.push((cond, l));
                    return;
                }
                if kind(l) == "null_literal" && kind(r) == "identifier" {
                    out.push((cond, r));
                    return;
                }
            }
            for c in named_children(cond) {
                null_comparisons(c, depth + 1, out);
            }
        }
        _ => {
            for c in named_children(cond) {
                null_comparisons(c, depth + 1, out);
            }
        }
    }
}

/// An unconditional dereference of `name` in an expression (see the rule description for
/// what is skipped).
fn deref_in<'t>(ctx: &Ctx<'t>, e: Node<'t>, name: &str, depth: u32) -> Option<Node<'t>> {
    if depth > 48 {
        return None;
    }
    let is_name = |n: Option<Node<'t>>| {
        n.map(syntax::strip_parens)
            .is_some_and(|n| kind(n) == "identifier" && ctx.text(n) == name)
    };
    let recurse_all =
        |e: Node<'t>| named_children(e).find_map(|c| deref_in(ctx, c, name, depth + 1));
    match kind(e) {
        "lambda_expression" | "class_body" | "switch_expression" | "ERROR" => None,
        "method_invocation" => {
            if is_name(e.field(Field::Object)) {
                return Some(e);
            }
            recurse_all(e)
        }
        "field_access" => {
            if is_name(e.field(Field::Object)) {
                return Some(e);
            }
            recurse_all(e)
        }
        "array_access" => {
            if is_name(e.field(Field::Array)) {
                return Some(e);
            }
            recurse_all(e)
        }
        "method_reference" => {
            if is_name(syntax::first_named_child(e)) {
                return Some(e);
            }
            None
        }
        "binary_expression" => {
            let op = syntax::operator_str(e);
            let left = e.field(Field::Left)?;
            if op == "&&" || op == "||" {
                deref_in(ctx, left, name, depth + 1)
            } else {
                deref_in(ctx, left, name, depth + 1)
                    .or_else(|| deref_in(ctx, e.field(Field::Right)?, name, depth + 1))
            }
        }
        "ternary_expression" => deref_in(ctx, e.field(Field::Condition)?, name, depth + 1),
        _ => recurse_all(e),
    }
}

/// The unconditional dereference of `name` made by statement `s`, if any.
fn deref_by_statement<'t>(ctx: &Ctx<'t>, s: Node<'t>, name: &str) -> Option<Node<'t>> {
    let is_name = |n: Option<Node<'t>>| {
        n.map(syntax::strip_parens)
            .is_some_and(|n| kind(n) == "identifier" && ctx.text(n) == name)
    };
    match kind(s) {
        "expression_statement" => deref_in(ctx, syntax::first_named_child(s)?, name, 0),
        "local_variable_declaration" => syntax::declarators(s)
            .filter_map(|d| d.field(Field::Value))
            .find_map(|v| deref_in(ctx, v, name, 0)),
        "if_statement" | "while_statement" | "switch_expression" => {
            deref_in(ctx, s.field(Field::Condition)?, name, 0)
        }
        "for_statement" => {
            // The initializer and the first test always run; the updates and the body may not.
            let cond = s.field(Field::Condition);
            named_children(s)
                .filter(|c| match cond {
                    Some(t) => c.start_byte() <= t.start_byte(),
                    None => kind(*c) == "local_variable_declaration",
                })
                .find_map(|c| deref_in(ctx, c, name, 0))
        }
        "enhanced_for_statement" => {
            let value = s.field(Field::Value)?;
            if is_name(Some(value)) {
                return Some(value);
            }
            deref_in(ctx, value, name, 0)
        }
        "synchronized_statement" => {
            let lock = named_children(s).find(|c| kind(*c) == "parenthesized_expression")?;
            if is_name(Some(lock)) {
                return Some(lock);
            }
            deref_in(ctx, lock, name, 0)
        }
        _ => None,
    }
}

/// Whether statement `s` assigns or declares `name` anywhere inside it.
fn touches(ctx: &Ctx<'_>, s: Node<'_>, name: &str) -> bool {
    if !ctx.text(s).contains(name) {
        return false;
    }
    let mut cursor = s.walk();
    let mut stack = vec![s];
    let mut steps = 0u32;
    while let Some(n) = stack.pop() {
        steps += 1;
        if steps > 20_000 {
            return true;
        }
        match kind(n) {
            "assignment_expression" => {
                if n.field(Field::Left)
                    .map(syntax::strip_parens)
                    .is_some_and(|l| kind(l) == "identifier" && ctx.text(l) == name)
                {
                    return true;
                }
            }
            "update_expression" => {
                if syntax::first_named_child(n)
                    .map(syntax::strip_parens)
                    .is_some_and(|l| kind(l) == "identifier" && ctx.text(l) == name)
                {
                    return true;
                }
            }
            "variable_declarator" | "formal_parameter" | "catch_formal_parameter" => {
                if n.field(Field::Name).is_some_and(|l| ctx.text(l) == name) {
                    return true;
                }
            }
            "enhanced_for_statement" => {
                if n.field(Field::Name).is_some_and(|l| ctx.text(l) == name) {
                    return true;
                }
            }
            "ERROR" => return true,
            _ => {}
        }
        for c in n.named_children(&mut cursor) {
            stack.push(c);
        }
    }
    false
}

impl<'t> JavaRule<'t> for NullCheckAfterDereference {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["if_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(cond) = node.field(Field::Condition) else {
            return;
        };
        if !ctx.text(cond).contains("null") {
            return;
        }
        let Some(block) = ctx.parent(node) else {
            return;
        };
        if !matches!(kind(block), "block" | "constructor_body") {
            return;
        }
        let mut checks = Vec::new();
        null_comparisons(cond, 0, &mut checks);
        if checks.is_empty() {
            return;
        }
        // The statements before the `if` in its block, nearest last.
        let mut before: Vec<Node<'t>> = Vec::new();
        for s in named_children(block) {
            if s.id() == node.id() {
                break;
            }
            before.push(s);
        }
        let start = before.len().saturating_sub(MAX_STATEMENTS);
        let before = &before[start..];
        for (cmp, ident) in checks {
            let name = ctx.text(ident);
            let mut found = None;
            for &s in before.iter().rev() {
                if !ctx.text(s).contains(name) {
                    continue;
                }
                if touches(ctx, s, name) {
                    break;
                }
                if let Some(d) = deref_by_statement(ctx, s, name) {
                    found = Some(d);
                    break;
                }
            }
            let Some(deref) = found else { continue };
            let Some(decl) = ctx.declaration_of(ident) else {
                continue;
            };
            if !matches!(
                decl.kind,
                VarKind::Local | VarKind::Parameter | VarKind::ForEachVariable
            ) {
                continue;
            }
            ctx.emit(
                cmp,
                format!(
                    "`{name}` is compared with null after line {} already dereferenced it: move the check up or remove it",
                    ctx.line(deref)
                ),
            )
            .secondary(deref, "dereferenced here");
        }
    }
}
