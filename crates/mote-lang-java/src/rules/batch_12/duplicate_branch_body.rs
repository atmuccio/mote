//! `java/duplicate-branch-body` — two branches of an `else if` chain, or two groups of a
//! `switch`, that run the same code.

use super::util::{any_descendant, keyword, same_code};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["suspicious", "copy-paste"],
    params: &[ParamSpec {
        name: "min_statements",
        default: ParamDefault::Int(2),
        description: "Smallest number of statements (a trailing `break` not counted) a duplicated branch must have to be reported.",
    }],
    remediation_minutes: 10,
    description: r#"Detects a `switch` group whose code is the same as that of an earlier group of the
same `switch`, and an `else if` branch whose code is the same as that of the branch right
before it.

**Why it matters.** Two different cases leading to the same several statements is usually a
copy-paste slip: the second branch was pasted from the first and never adapted, so the second
case silently gets the first case's behaviour (a log message naming the wrong state, a value
read from the wrong source). When the duplication is deliberate, it still hides the fact that
the two cases are handled alike, and the copies drift apart the first time only one of them is
fixed.

**How to fix.** Adapt the branch that was meant to differ. If both cases really are handled
the same way, merge them: `if (a) X else if (b) X` becomes `if (a || b) X`, and two `switch`
groups become one group with both labels.

```java
// Noncompliant
switch (status) {
    case COMMITTED:
        log.warn("unexpected COMMITTED state for " + tx);
        return false;
    case COMMITTING:
        log.warn("unexpected COMMITTED state for " + tx);   // pasted, never adapted
        return false;
    default:
        return true;
}

// Compliant
switch (status) {
    case COMMITTED:
        log.warn("unexpected COMMITTED state for " + tx);
        return false;
    case COMMITTING:
        log.warn("unexpected COMMITTING state for " + tx);
        return false;
    default:
        return true;
}
```

Code is compared token by token, ignoring whitespace and comments. Branches shorter than
`min_statements` statements (default 2, a trailing `break` not counted) are not reported:
one-line branches such as `return true;` repeat on purpose in table-like code. In an `else if`
chain only neighbouring branches are compared, since a later branch may depend on the
conditions tested in between; the last `if`/`else` pair is left to `identical-branches`.
`switch` groups that fall through into the next one, and chains or switches whose conditions
bind pattern variables (the same text can then call different overloads), are not compared."#,
    ..RuleMeta::new(
        "java/duplicate-branch-body",
        "java",
        "Branch repeats the code of another branch",
    )
};

pub struct DuplicateBranchBody;

impl<'t> JavaRule<'t> for DuplicateBranchBody {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["if_statement", "switch_block"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let min = ctx.param_i64("min_statements").max(1) as usize;
        if kind(node) == "if_statement" {
            check_chain(node, ctx, min);
        } else {
            check_switch(node, ctx, min);
        }
    }
}

/// One branch to compare: the code, where to report, and its size.
struct Branch<'t> {
    /// Statements of the branch (one element for a single statement or a block).
    stmts: Vec<Node<'t>>,
    /// Where a finding about this branch is reported.
    at: Node<'t>,
    /// Statements counted for `min_statements`.
    size: usize,
}

fn statement_count(body: Node<'_>) -> usize {
    if kind(body) == "block" {
        named_children(body).count()
    } else {
        1
    }
}

fn binds_pattern(cond: Node<'_>, src: &str) -> bool {
    syntax::node_text(cond, src).contains("instanceof")
        && any_descendant(cond, |n| {
            kind(n) == "instanceof_expression"
                && (n.field(Field::Name).is_some() || n.field(Field::Pattern).is_some())
        })
}

fn check_chain<'t>(node: Node<'t>, ctx: &Ctx<'t>, min: usize) {
    // Cheapest test first: most `if`s have no `else if`.
    if node
        .field(Field::Alternative)
        .is_none_or(|a| kind(a) != "if_statement")
    {
        return;
    }
    // Only the head of a chain; `else if` links are handled from it.
    if let Some(p) = ctx.parent(node) {
        if kind(p) == "if_statement"
            && p.field(Field::Alternative).map(|a| a.id()) == Some(node.id())
        {
            return;
        }
    }
    let mut branches: Vec<Branch<'t>> = Vec::new();
    let mut links = 0usize;
    let mut has_final_else = false;
    let mut cur = node;
    loop {
        let Some(then) = cur.field(Field::Consequence) else {
            return;
        };
        if cur
            .field(Field::Condition)
            .is_some_and(|c| binds_pattern(c, ctx.source()))
        {
            return;
        }
        branches.push(Branch {
            stmts: vec![then],
            at: keyword(cur, "if").unwrap_or(cur),
            size: statement_count(then),
        });
        let Some(alt) = cur.field(Field::Alternative) else {
            break;
        };
        if kind(alt) == "if_statement" {
            links += 1;
            cur = alt;
            continue;
        }
        branches.push(Branch {
            stmts: vec![alt],
            at: keyword(cur, "else").unwrap_or(alt),
            size: statement_count(alt),
        });
        has_final_else = true;
        break;
    }
    if links == 0 {
        return;
    }
    let n = branches.len();
    // The last `if`/`else` pair is `identical-branches`' business.
    let skip_pair = has_final_else.then(|| (n - 2, n - 1));
    // Only neighbours: merging `if (a) X else if (b) X` into `if (a || b) X` is always safe,
    // while a branch further down may rely on the conditions in between.
    report_duplicates(&branches, skip_pair, true, ctx, min, "branch");
}

fn is_jump(stmt: Node<'_>) -> bool {
    matches!(
        kind(stmt),
        "break_statement"
            | "continue_statement"
            | "return_statement"
            | "throw_statement"
            | "yield_statement"
    )
}

fn check_switch<'t>(block: Node<'t>, ctx: &Ctx<'t>, min: usize) {
    let mut branches: Vec<Branch<'t>> = Vec::new();
    for group in named_children(block) {
        let k = kind(group);
        if k != "switch_block_statement_group" && k != "switch_rule" {
            continue;
        }
        let mut label = None;
        let mut stmts = Vec::new();
        for c in named_children(group) {
            if kind(c) == "switch_label" {
                if any_descendant(c, |n| {
                    matches!(
                        kind(n),
                        "pattern" | "type_pattern" | "record_pattern" | "guard"
                    )
                }) {
                    return;
                }
                label.get_or_insert(c);
            } else {
                stmts.push(c);
            }
        }
        let (Some(label), Some(last)) = (label, stmts.last()) else {
            continue;
        };
        // A group that falls through runs more than its own statements.
        if k == "switch_block_statement_group" && !is_jump(*last) {
            continue;
        }
        let size = if k == "switch_rule" {
            statement_count(*last)
        } else {
            stmts.len() - usize::from(kind(*last) == "break_statement")
        };
        branches.push(Branch {
            stmts,
            at: label,
            size,
        });
    }
    report_duplicates(&branches, None, false, ctx, min, "case");
}

fn same_branch(a: &Branch<'_>, b: &Branch<'_>, src: &str) -> bool {
    a.stmts.len() == b.stmts.len()
        && a.stmts
            .iter()
            .zip(&b.stmts)
            .all(|(x, y)| same_code(*x, *y, src))
}

fn report_duplicates<'t>(
    branches: &[Branch<'t>],
    skip_pair: Option<(usize, usize)>,
    adjacent_only: bool,
    ctx: &Ctx<'t>,
    min: usize,
    what: &str,
) {
    let src = ctx.source();
    for j in 1..branches.len() {
        let b = &branches[j];
        if b.size < min {
            continue;
        }
        let from = if adjacent_only { j - 1 } else { 0 };
        let first = (from..j).find(|&i| {
            skip_pair != Some((i, j))
                && branches[i].size == b.size
                && same_branch(&branches[i], b, src)
        });
        if let Some(i) = first {
            let line = ctx.line(branches[i].at);
            ctx.emit(
                b.at,
                format!(
                    "This {what} runs the same code as the {what} at line {line}; fix the copy or merge the two"
                ),
            )
            .secondary(branches[i].at, "first copy");
        }
    }
}
