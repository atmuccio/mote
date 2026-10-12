//! `java/for-loop-counter-misuse` — a `for` header that does not govern its loop: the body
//! assigns the counter, or the update changes none of the variables the condition tests.

use super::util;
use crate::prelude::*;
use crate::rules::shared::util::CoveredRanges;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["loops", "confusing"],
    remediation_minutes: 10,
    description: r#"Detects `for` loops whose header does not say how the loop advances:

* the body assigns the counter (`i = …`) although the update clause changes it too, or steps
  it further (`i += 2`) on every pass, so the loop visits other elements than the header
  promises;
* the update clause changes only variables that the condition does not test
  (`for (int i = 0; i < n; j++)`), or the counter of an enclosing loop
  (`for (int col = 0; col < grid[row].length; row++)`), usually a slip when copying a nested
  loop.

**Why it matters.** The three parts of a `for` header are where readers look to learn which
values the counter takes. When the body moves the counter as well, that promise is wrong:
elements are skipped or visited twice depending on data, off-by-one errors hide in the
interplay of the two updates, and the loop's end is harder to prove. An update that doesn't
touch what the condition tests either never ends the loop or leaves the real advance to the
body.

**How to fix.** Let one place control the counter: use a `while` loop when the step depends on
the data, or restructure the body so that the header's update is the only change.

```java
// Noncompliant
for (int i = 0; i < records.length; i++) {
    process(records[i]);
    i += 2;                       // really a step of 3
}
for (int i = 0; i < size; i++) {
    if (queue.get(i).matches(id)) {
        found = true;
        i = size;                 // a disguised break
    }
}

// Compliant
for (int i = 0; i < records.length; i += 3) {
    process(records[i]);
}
for (int i = 0; i < size; i++) {
    if (queue.get(i).matches(id)) {
        found = true;
        break;
    }
}
```

Only counters declared in the loop header and compared with `<`, `<=`, `>` or `>=` are
considered (loops tested with `==` or `!=` are left to rules about skipping the bound). Not
reported: `++` and `--` on the counter in the body (the idiom that consumes the next element
while parsing, or that steps back after removing an element), compound steps (`i += 2`)
taken only under a condition (the same parsing idiom for longer tokens), writes inside a
lambda or an inner class, and loops whose update clause is empty (then the body is the only
place that advances the counter, which the header shows plainly)."#,
    ..RuleMeta::new(
        "java/for-loop-counter-misuse",
        "java",
        "for loop counter changed outside its header",
    )
};

#[derive(Default)]
pub struct ForLoopCounterMisuse<'t> {
    /// Start offsets of the assignments to a plain name, by name, in order, in the loop bodies
    /// indexed so far ([`Self::covered`]): a body is indexed by the first loop that needs it
    /// (an enclosing loop's index already holds a nested body's), so that a loop whose body
    /// assigns none of its counters is skipped without visiting the body, and nested loops do
    /// not visit every inner body again. Only the bodies of loops that get that far are
    /// visited, not the whole file.
    assignments: FxHashMap<&'t str, Vec<usize>>,
    covered: CoveredRanges,
}

impl<'t> ForLoopCounterMisuse<'t> {
    /// Makes sure the assignments in `body` are indexed.
    fn index_body(&mut self, ctx: &Ctx<'t>, body: Node<'t>) {
        let (from, to) = (body.start_byte(), body.end_byte());
        if self.covered.contains(from, to) {
            return;
        }
        let in_order = self.covered.add(from, to);
        let mut cursor = body.walk();
        'walk: loop {
            let n = cursor.node();
            if is_assignment(n) {
                if let Some((name, _)) = written(ctx, n) {
                    self.assignments
                        .entry(name)
                        .or_default()
                        .push(n.start_byte());
                }
            }
            if cursor.goto_first_child() {
                continue;
            }
            loop {
                if cursor.goto_next_sibling() {
                    continue 'walk;
                }
                if !cursor.goto_parent() || cursor.node().id() == body.id() {
                    break 'walk;
                }
            }
        }
        if !in_order {
            // A body before one indexed earlier (a loop inside the header of another loop):
            // keep every list in source order.
            for starts in self.assignments.values_mut() {
                starts.sort_unstable();
            }
        }
    }
}

/// Whether `n` is an `assignment_expression` (by kind id: this runs on every node of a body).
fn is_assignment(n: Node<'_>) -> bool {
    static IDS: std::sync::LazyLock<Vec<u16>> =
        std::sync::LazyLock::new(|| crate::kinds::ids_for_kind("assignment_expression", true));
    IDS.contains(&n.kind_id())
}

/// Names declared in the `init` of a `for` statement.
fn declared_counters<'t>(ctx: &Ctx<'t>, node: Node<'t>) -> Vec<&'t str> {
    let mut out = Vec::new();
    let mut cursor = node.walk();
    for init in node.children_by_field_name("init", &mut cursor) {
        if kind(init) != "local_variable_declaration" {
            continue;
        }
        for d in syntax::declarators(init) {
            if let Some(n) = d.field(Field::Name) {
                out.push(ctx.text(n));
            }
        }
    }
    out
}

/// The identifier an update expression or assignment writes, if any.
fn written<'t>(ctx: &Ctx<'t>, e: Node<'t>) -> Option<(&'t str, bool)> {
    let e = syntax::strip_parens(e);
    match kind(e) {
        "update_expression" => {
            let operand = syntax::first_named_child(e)?;
            (kind(operand) == "identifier").then(|| (ctx.text(operand), true))
        }
        "assignment_expression" => {
            let left = e.field(Field::Left)?;
            (kind(left) == "identifier").then(|| (ctx.text(left), false))
        }
        _ => None,
    }
}

/// The variables whose values the condition compares: identifiers in its operands, except
/// those only used to pick an element (`row` in `grid[row].length`) or passed to a call.
fn tested_values<'t>(ctx: &Ctx<'t>, cond: Node<'t>) -> Vec<&'t str> {
    let mut out = Vec::new();
    collect_values(ctx, cond, &mut out, 0);
    out
}

fn collect_values<'t>(ctx: &Ctx<'t>, n: Node<'t>, out: &mut Vec<&'t str>, depth: u32) {
    if depth > 64 {
        return;
    }
    match kind(n) {
        "identifier" => out.push(ctx.text(n)),
        "argument_list" | "lambda_expression" | "class_body" => {}
        "array_access" => {
            if let Some(a) = n.field(Field::Array) {
                collect_values(ctx, a, out, depth + 1);
            }
        }
        "field_access" => {
            if let Some(o) = n.field(Field::Object) {
                collect_values(ctx, o, out, depth + 1);
            }
        }
        "method_invocation" => {
            if let Some(o) = n.field(Field::Object) {
                collect_values(ctx, o, out, depth + 1);
            }
        }
        _ => {
            for c in named_children(n) {
                collect_values(ctx, c, out, depth + 1);
            }
        }
    }
}

impl<'t> JavaRule<'t> for ForLoopCounterMisuse<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["for_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(cond) = node.field(Field::Condition).map(syntax::strip_parens) else {
            return;
        };
        if kind(cond) != "binary_expression"
            || !matches!(syntax::operator_str(cond), "<" | "<=" | ">" | ">=")
        {
            return;
        }
        let counters = declared_counters(ctx, node);
        if counters.is_empty() {
            return;
        }
        // Updates: every one must be a plain write of an identifier.
        let mut updates: Vec<(&'t str, Node<'t>)> = Vec::new();
        {
            let mut cursor = node.walk();
            for u in node.children_by_field_name("update", &mut cursor) {
                if syntax::is_comment(u) {
                    continue;
                }
                let Some((name, _)) = written(ctx, u) else {
                    return;
                };
                updates.push((name, u));
            }
        }
        if updates.is_empty() {
            return;
        }
        // Counters the condition tests.
        let tested: Vec<&'t str> = counters
            .iter()
            .copied()
            .filter(|c| util::mentions(ctx, cond, c))
            .collect();
        if tested.is_empty() {
            return;
        }
        // The update governs the loop when it changes a counter of this header that the
        // condition tests, or anything else the condition compares (`i < k` with `k--`); a
        // variable the condition only uses as an index counts unless it is the counter of an
        // enclosing loop (`col < grid[row].length` with `row++` is a copy slip).
        let values = tested_values(ctx, cond);
        // Only needed for an update that changes neither (rare): walking the enclosing loops of
        // every loop is quadratic in the nesting.
        let mut outer: Option<Vec<&'t str>> = None;
        let updated_tested = updates.iter().any(|(name, _)| {
            tested.contains(name)
                || values.contains(name)
                || (util::mentions(ctx, cond, name)
                    && !outer
                        .get_or_insert_with(|| enclosing_counters(ctx, node))
                        .contains(name))
        });
        if !updated_tested {
            // The update changes nothing the condition looks at.
            let (first, last) = (updates[0].1, updates[updates.len() - 1].1);
            let names: Vec<&str> = updates.iter().map(|(n, _)| *n).collect();
            ctx.emit_range(
                Span::cover(first, last),
                format!(
                    "This update changes `{}`, which the loop condition does not test; update \
                     `{}` here or use a while loop",
                    names.join("`, `"),
                    tested[0]
                ),
            );
            return;
        }
        // Body writes (other than ++/--) to a tested counter that the update also changes.
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        let governed: Vec<&'t str> = tested
            .iter()
            .copied()
            .filter(|c| updates.iter().any(|(n, _)| n == c))
            .collect();
        if governed.is_empty() {
            return;
        }
        // Only an assignment of a governed counter in the body can be reported.
        let (from, to) = (body.start_byte(), body.end_byte());
        if from >= to {
            return;
        }
        self.index_body(ctx, body);
        let index = &self.assignments;
        let assigned = governed.iter().any(|c| {
            index.get(c).is_some_and(|starts| {
                let k = starts.partition_point(|&s| s < from);
                starts.get(k).is_some_and(|&s| s < to)
            })
        });
        if !assigned {
            return;
        }
        // Plain assignments anywhere; compound steps only when every pass takes them (a step
        // taken under a condition is the parsing idiom that skips what was just consumed).
        let mut hit: Option<(Node<'t>, &'t str)> = None;
        visit_writes(body, &mut |n, conditional| {
            if kind(n) != "assignment_expression" {
                return;
            }
            let Some((name, _)) = written(ctx, n) else {
                return;
            };
            if !governed.contains(&name) || hit.is_some() {
                return;
            }
            if syntax::operator_str(n) == "=" || !(conditional || jumps_before(body, n)) {
                hit = Some((n, name));
            }
        });
        if let Some((write, name)) = hit {
            let msg = if syntax::operator_str(write) == "=" {
                format!(
                    "Don't assign the loop counter `{name}` in the body of a for loop that \
                     updates it in its header; use `break` or a while loop"
                )
            } else {
                format!(
                    "Don't step the loop counter `{name}` in the body of a for loop that updates \
                     it in its header; put the whole step in the header"
                )
            };
            ctx.emit(write, msg);
        }
    }
}

/// Visits the nodes of a loop body (not entering lambdas and class bodies), telling for each
/// whether it runs only under a condition: inside an `if`, `switch`, conditional expression,
/// `&&` / `||` operand, nested loop or catch block.
fn visit_writes<'t>(body: Node<'t>, f: &mut impl FnMut(Node<'t>, bool)) {
    fn go<'t>(n: Node<'t>, conditional: bool, depth: u32, f: &mut impl FnMut(Node<'t>, bool)) {
        if depth > 200 {
            return;
        }
        for c in named_children(n) {
            if util::is_other_context(c) {
                continue;
            }
            f(c, conditional);
            let inner = conditional
                || match kind(c) {
                    "if_statement"
                    | "switch_expression"
                    | "switch_block_statement_group"
                    | "switch_rule"
                    | "ternary_expression"
                    | "while_statement"
                    | "do_statement"
                    | "for_statement"
                    | "enhanced_for_statement"
                    | "catch_clause" => true,
                    "binary_expression" => {
                        matches!(syntax::operator_str(c), "&&" | "||")
                    }
                    _ => false,
                };
            go(c, inner, depth + 1, f);
        }
    }
    go(body, false, 0, f);
}

/// Whether a `continue`, `break`, `return` or `throw` of the loop body comes before `write`:
/// then some passes leave before reaching it, and the step is conditional after all.
fn jumps_before<'t>(body: Node<'t>, write: Node<'t>) -> bool {
    let mut found = false;
    util::visit(body, |n| {
        if n.start_byte() >= write.start_byte() {
            return util::Walk::Stop;
        }
        match kind(n) {
            "lambda_expression" | "class_body" => util::Walk::Skip,
            "continue_statement" | "break_statement" | "return_statement" | "throw_statement" => {
                found = true;
                util::Walk::Stop
            }
            _ => util::Walk::Continue,
        }
    });
    found
}

/// Enclosing `for` loops [`enclosing_counters`] looks at: real code nests a handful, and an
/// unbounded walk per loop is quadratic in machine-made nesting.
const MAX_ENCLOSING_LOOPS: usize = 64;

/// The counters declared in the headers of the `for` loops enclosing `node` (up to the
/// enclosing function, and at most [`MAX_ENCLOSING_LOOPS`] loops out).
fn enclosing_counters<'t>(ctx: &Ctx<'t>, node: Node<'t>) -> Vec<&'t str> {
    let mut out = Vec::new();
    let mut loops = 0;
    for a in ctx.ancestors(node) {
        match kind(a) {
            "for_statement" => {
                loops += 1;
                if loops > MAX_ENCLOSING_LOOPS {
                    break;
                }
                out.extend(declared_counters(ctx, a));
            }
            "method_declaration"
            | "constructor_declaration"
            | "lambda_expression"
            | "class_body"
            | "static_initializer" => break,
            _ => {}
        }
    }
    out
}
