//! `java/string-concat-in-loop` — building a `String` with `+=` inside a loop.

use super::util::{self, Fld, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Medium,
    tags: &["performance", "strings"],
    remediation_minutes: 10,
    description: r#"Detects `s += …` and `s = s + …` inside a loop body when `s` is a `String`
declared outside that loop.

**Why it matters.** Strings are immutable: every concatenation copies the whole accumulated
text into a new object. Growing a string over *n* iterations therefore costs O(n²) time and
produces a lot of garbage, which becomes noticeable on large inputs (building SQL, CSV,
reports, log lines).

**How to fix.** Accumulate in a `StringBuilder` (or `StringJoiner` / `String.join` /
`Collectors.joining` when there is a separator) and convert once after the loop.

```java
// Noncompliant
String csv = "";
for (String field : fields) {
    csv += field + ",";
}

// Compliant
StringBuilder csv = new StringBuilder();
for (String field : fields) {
    csv.append(field).append(',');
}
String result = csv.toString();
```

Each variable is reported once per loop. Not reported: variables declared inside the loop
body and the variable of an enhanced `for` (they don't accumulate across iterations), a concatenation immediately followed by `break`, `return`, or `throw` (it runs at
most once), a concatenation guarded by a counter test (`if (n == 0) { s = s + x; }` with
`n++` in the loop: it runs on one pass at most), assignments in the loop header, and targets
whose type is unknown."#,
    ..RuleMeta::new(
        "java/string-concat-in-loop",
        "java",
        "String concatenation in a loop",
    )
};

/// Reports each variable once per loop.
#[derive(Default)]
pub struct StringConcatInLoop<'t> {
    reported: FxHashSet<(usize, &'t str)>,
}

/// Whether `expr` is a `+` chain with `target` as one of its operands (`s + a + b`, `a + s`).
fn concat_chain_contains(expr: Node<'_>, target: Node<'_>, src: &str) -> bool {
    let mut cur = syntax::strip_parens(expr);
    if !(kind(cur) == "binary_expression" && util::op_str(cur) == "+") {
        return false;
    }
    let mut steps = 0;
    loop {
        steps += 1;
        if steps > 10_000 {
            return false;
        }
        if kind(cur) == "binary_expression" && util::op_str(cur) == "+" {
            if cur
                .fld(F::Right)
                .is_some_and(|r| util::same_code(r, target, src))
            {
                return true;
            }
            match cur.fld(F::Left) {
                Some(l) => cur = syntax::strip_parens(l),
                None => return false,
            }
        } else {
            return util::same_code(cur, target, src);
        }
    }
}

/// Whether the assignment sits in the `then` branch of an `if (n == 0)` (or any other whole
/// number) inside the loop, and the loop increments `n` (`n++`, `++n`, `n += …`): the branch
/// runs on one pass only.
fn first_pass_only<'t>(ctx: &Ctx<'t>, node: Node<'t>, loop_: Node<'t>) -> bool {
    let mut prev = node;
    for a in ctx.ancestors(node) {
        if a.id() == loop_.id() {
            return false;
        }
        if kind(a) == "if_statement" && a.fld(F::Consequence).is_some_and(|c| c.id() == prev.id()) {
            if let Some(counter) = a
                .fld(F::Condition)
                .map(syntax::strip_parens)
                .and_then(|c| zero_test(ctx, c))
            {
                let body: String = ctx
                    .text(loop_)
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .collect();
                if [
                    format!("{counter}++"),
                    format!("++{counter}"),
                    format!("{counter}+="),
                ]
                .iter()
                .any(|p| body.contains(p.as_str()))
                {
                    return true;
                }
            }
        }
        prev = a;
    }
    false
}

/// `n == 0` / `1 == n`: the counter's name. With a counter that only grows, such a branch
/// runs on one pass at most.
fn zero_test<'t>(ctx: &Ctx<'t>, cond: Node<'t>) -> Option<&'t str> {
    if kind(cond) != "binary_expression" || util::op_str(cond) != "==" {
        return None;
    }
    let (l, r) = (cond.fld(F::Left)?, cond.fld(F::Right)?);
    let int = |n: Node<'_>| kind(n) == "decimal_integer_literal";
    match (kind(l), kind(r)) {
        ("identifier", _) if int(r) => Some(ctx.text(l)),
        (_, "identifier") if int(l) => Some(ctx.text(r)),
        _ => None,
    }
}

impl<'t> JavaRule<'t> for StringConcatInLoop<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["assignment_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let op = util::op_str(node);
        if op != "+=" && op != "=" {
            return;
        }
        let (Some(left), Some(right)) = (node.fld(F::Left), node.fld(F::Right)) else {
            return;
        };
        // The accumulated variable: a name, or `this.name`.
        let ident = match kind(left) {
            "identifier" => left,
            "field_access" => {
                if left.fld(F::Object).is_none_or(|o| kind(o) != "this") {
                    return;
                }
                match left.fld(F::Field) {
                    Some(f) => f,
                    None => return,
                }
            }
            _ => return,
        };
        if op == "=" && !concat_chain_contains(right, left, ctx.source()) {
            return;
        }
        let Some(loop_) = util::enclosing_loop_body(ctx, node) else {
            return;
        };
        // Runs at most once when the next statement leaves the loop.
        if let Some(stmt) = ctx
            .parent(node)
            .filter(|p| kind(*p) == "expression_statement")
        {
            if util::next_statement(ctx, stmt).is_some_and(|n| {
                matches!(
                    kind(n),
                    "break_statement" | "return_statement" | "throw_statement"
                )
            }) {
                return;
            }
        }
        if first_pass_only(ctx, node, loop_) {
            return;
        }
        let Some(t) = ctx.type_of(left) else { return };
        if !t.is_string() {
            return;
        }
        if kind(left) == "identifier" {
            let Some(decl) = ctx.declaration_of(ident) else {
                return;
            };
            let d = decl.decl;
            let body = loop_.fld(F::Body).unwrap_or(loop_);
            if d.start_byte() >= body.start_byte() && d.end_byte() <= body.end_byte() {
                return; // declared inside the loop: a fresh string per iteration
            }
            if decl.kind == VarKind::ForEachVariable && d.id() == loop_.id() {
                return; // the loop variable itself: reassigned on every iteration
            }
        }
        let name = ctx.text(ident);
        if !self.reported.insert((loop_.id(), name)) {
            return;
        }
        ctx.emit(
            left,
            format!("Build `{name}` with a StringBuilder; concatenating in a loop copies the whole string each time"),
        );
    }
}
