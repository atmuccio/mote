//! `java/constant-if-condition` — `if (true)` / `if (false)`.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["suspicious", "dead-code"],
    remediation_minutes: 2,
    description: r#"Detects `if` statements whose condition is the literal `true` or `false`.

**Why it matters.** The branch is decided at compile time: with `if (true)` the `else` part is
dead and the test is noise; with `if (false)` the body never runs. Such conditions are usually
leftovers of debugging or of switching code off by hand, and they hide real logic: a reader
has to notice the literal to know that half of the statement is dead.

**How to fix.** Keep only the branch that runs, delete the dead one, or restore the real
condition. To switch behavior on and off, use a named configuration flag.

```java
// Noncompliant
if (false) {
    audit(order);
}
if (true) {
    ship(order);
} else {
    hold(order);
}

// Compliant
ship(order);
```

Not reported: conditions that merely *contain* a literal (`if (DEBUG && ready)`), named
constants such as `if (DEBUG)` (a deliberate compile-time switch), and loops (`while (true)`
is the usual way to write a loop that exits from inside). Also not reported:

* `if (false)` whose body only throws, with no `else`. That is a known way to make `javac`
  accept a `catch` of a checked exception that the `try` block throws without declaring it
  (a call into Kotlin code, a sneaky throw):
  `try { kotlinApi(); if (false) { throw new IOException(); } } catch (IOException e) { … }`.
* A constant condition explained by a comment: an own-line comment directly above the `if`
  (no blank line between), or an own-line comment inside one of its blocks. A comment at the
  end of the `if` line itself does not count."#,
    ..RuleMeta::new(
        "java/constant-if-condition",
        "java",
        "if condition is a boolean literal",
    )
};

pub struct ConstantIfCondition;

impl<'t> JavaRule<'t> for ConstantIfCondition {
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
        let lit = syntax::strip_parens(cond);
        let message = match kind(lit) {
            "true" => "Remove this `if (true)`: the condition is always true",
            "false" => "Remove this `if (false)` or restore a real condition: its body never runs",
            _ => return,
        };
        if kind(lit) == "false" && only_throws(node) {
            return;
        }
        if explained_by_comment(node, ctx) {
            return;
        }
        ctx.emit(lit, message);
    }
}

/// `if (…) throw …;` or `if (…) { throw …; … }` holding nothing but `throw` statements, without
/// an `else`.
fn only_throws(if_stmt: Node<'_>) -> bool {
    if if_stmt.field(Field::Alternative).is_some() {
        return false;
    }
    let Some(then) = if_stmt.field(Field::Consequence) else {
        return false;
    };
    match kind(then) {
        "throw_statement" => true,
        "block" => {
            let mut any = false;
            for s in named_children(then) {
                if kind(s) != "throw_statement" {
                    return false;
                }
                any = true;
            }
            any
        }
        _ => false,
    }
}

/// Whether `comment` is the first thing on its line (not trailing code).
fn is_own_line(comment: Node<'_>, src: &str) -> bool {
    let before = src.get(..comment.start_byte()).unwrap_or("");
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    before[line_start..].trim().is_empty()
}

/// An own-line comment directly above the `if` (the line before it), or an own-line comment
/// among the statements of its `then` or `else` block.
fn explained_by_comment<'t>(if_stmt: Node<'t>, ctx: &Ctx<'t>) -> bool {
    let src = ctx.source();
    if let Some(prev) = ctx.prev_named_sibling(if_stmt) {
        if syntax::is_comment(prev)
            && prev.end_position().row + 1 == if_stmt.start_position().row
            && is_own_line(prev, src)
        {
            return true;
        }
    }
    [Field::Consequence, Field::Alternative]
        .into_iter()
        .filter_map(|f| if_stmt.field(f))
        .filter(|b| kind(*b) == "block")
        .any(|b| syntax::children(b).any(|c| syntax::is_comment(c) && is_own_line(c, src)))
}
