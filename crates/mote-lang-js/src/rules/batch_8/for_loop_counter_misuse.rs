//! `js/for-loop-counter-misuse`

use super::util;
use crate::prelude::*;

pub struct ForLoopCounterMisuse;

pub static FOR_LOOP_COUNTER_MISUSE: RuleMeta = RuleMeta {
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["loops", "confusing"],
    remediation_minutes: 10,
    description: r#"Flags `for` loops whose header does not govern the loop: the update clause changes none of
the variables the condition tests, or the body assigns a new value to the loop counter.

**Why:** readers take a `for` header as the full description of the iteration: where it starts,
when it stops, how it advances. When the update advances some other variable (`j++` in a loop over
`i`), the condition never changes and the loop only ends through a `break`, or never; this is
almost always a typo. When the body reassigns the counter, the header no longer tells the truth,
and an off-by-one hides easily in the interplay of the two updates.

**Fix:** update the variable the condition tests; when the body has to move the position itself,
use a `while` loop so that all the moves are in one place.

```js
for (let i = 0; i < rows.length; j++) {   // flagged: j++ never changes i
  total += rows[i];
}

for (let i = 0; i < parts.length; i++) {
  if (parts[i] === '--') {
    i = parts.length;                     // flagged: the body moves the counter
  }
}

for (let i = 0; i < rows.length; i++) {   // compliant
  total += rows[i];
}
```

Not reported: `++`, `--`, compound assignments (`i += 2`) and their long forms (`i = i + 2`) of the
counter in the body, which skip elements by a fixed step (a common parsing idiom), assignments inside nested functions,
conditions that call functions or use `this`, and conditions whose values the body changes
(writing a tested variable, calling a method on it or passing it to a function).
"#,
    ..RuleMeta::new(
        "js/for-loop-counter-misuse",
        "js",
        "A for loop's header should govern its iteration",
    )
};

impl JsRule for ForLoopCounterMisuse {
    fn meta(&self) -> &'static RuleMeta {
        &FOR_LOOP_COUNTER_MISUSE
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::ForStatement]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::ForStatement(stmt) = node.kind() else {
            return;
        };
        let (Some(test), Some(update)) = (&stmt.test, &stmt.update) else {
            return;
        };
        let (test_span, update_span, body_span) = (test.span(), update.span(), stmt.body.span());

        let mut test_names: Vec<&str> = Vec::new();
        let mut test_symbols: Vec<SymbolId> = Vec::new();
        let mut test_is_simple = true;
        let mut update_names: Vec<&str> = Vec::new();
        let mut update_symbols: Vec<SymbolId> = Vec::new();
        let mut update_is_simple = true;
        // Body facts.
        let mut body_touches_test = false;
        let mut counter_assignments: Vec<(SymbolId, Span, &str)> = Vec::new();
        let mut skip_until = 0u32;

        for inner in util::descendants(ctx, node) {
            let span = inner.span();
            let in_test = util::within(span, test_span);
            let in_update = util::within(span, update_span);
            let in_body = util::within(span, body_span);
            if in_test {
                match inner.kind() {
                    AstKind::CallExpression(_)
                    | AstKind::NewExpression(_)
                    | AstKind::ThisExpression(_)
                    | AstKind::Super(_)
                    | AstKind::AwaitExpression(_)
                    | AstKind::YieldExpression(_)
                    | AstKind::AssignmentExpression(_)
                    | AstKind::UpdateExpression(_)
                    | AstKind::TaggedTemplateExpression(_)
                    | AstKind::ComputedMemberExpression(_)
                    | AstKind::PrivateFieldExpression(_)
                    | AstKind::ChainExpression(_) => test_is_simple = false,
                    AstKind::IdentifierReference(id) => {
                        test_names.push(id.name.as_str());
                        if let Some(symbol) = ctx.reference_symbol(id) {
                            test_symbols.push(symbol);
                        }
                    }
                    _ => {}
                }
                continue;
            }
            if in_update {
                match inner.kind() {
                    AstKind::CallExpression(_)
                    | AstKind::NewExpression(_)
                    | AstKind::TaggedTemplateExpression(_) => update_is_simple = false,
                    AstKind::IdentifierReference(id) if is_written(ctx, id) => {
                        update_names.push(id.name.as_str());
                        if let Some(symbol) = ctx.reference_symbol(id) {
                            update_symbols.push(symbol);
                        }
                    }
                    _ => {}
                }
                continue;
            }
            if !in_body {
                continue;
            }
            let AstKind::IdentifierReference(id) = inner.kind() else {
                continue;
            };
            let name = id.name.as_str();
            if test_names.contains(&name) && (is_written(ctx, id) || feeds_call(ctx, inner)) {
                body_touches_test = true;
            }
            if span.start < skip_until {
                continue;
            }
            // Assignments in nested functions run at another time: skip them.
            if let Some(f) = ctx.enclosing_function(inner) {
                if f.span().start > node.span().start {
                    skip_until = skip_until.max(f.span().end);
                    continue;
                }
            }
            if let Some(AstKind::AssignmentExpression(a)) = ctx.parent_kind(inner) {
                if a.operator == AssignmentOperator::Assign
                    && a.left.span() == id.span
                    && !is_fixed_step(&a.right, name)
                {
                    if let Some(symbol) = ctx.reference_symbol(id) {
                        counter_assignments.push((symbol, a.span, name));
                    }
                }
            }
        }

        if !update_is_simple || update_names.is_empty() || test_names.is_empty() {
            return;
        }
        let governs = update_names.iter().any(|n| test_names.contains(n));
        if !governs {
            if test_is_simple && !body_touches_test {
                ctx.emit(
                    update_span,
                    format!(
                        "Update the variable the condition tests: '{}' does not change '{}'",
                        ctx.text(update_span),
                        ctx.text(test_span)
                    ),
                );
            }
            return;
        }
        for (symbol, span, name) in counter_assignments {
            if update_symbols.contains(&symbol) && test_symbols.contains(&symbol) {
                ctx.emit(
                    span,
                    format!(
                        "Do not assign the loop counter '{name}' in the body: the for header should govern it"
                    ),
                )
                .secondary(update_span, "The header's update");
            }
        }
    }
}

/// `i = i + 1`, `i = 2 + i`, `i = i - n`: the same fixed step a compound assignment takes.
fn is_fixed_step(value: &Expression<'_>, counter: &str) -> bool {
    let Expression::BinaryExpression(b) = helpers::unwrap_expr(value) else {
        return false;
    };
    let is_counter = |e: &Expression<'_>| matches!(helpers::unwrap_expr(e), Expression::Identifier(id) if id.name == counter);
    match b.operator {
        BinaryOperator::Addition => is_counter(&b.left) || is_counter(&b.right),
        BinaryOperator::Subtraction => is_counter(&b.left),
        _ => false,
    }
}

/// Is the identifier written (assignment or update)?
fn is_written(ctx: &Ctx<'_>, id: &IdentifierReference<'_>) -> bool {
    id.reference_id
        .get()
        .is_some_and(|r| ctx.scoping().get_reference(r).is_write())
}

/// Is the identifier a call argument or the receiver of a method call (`x.m()`, `f(x)`), which
/// may change what the condition sees?
fn feeds_call(ctx: &Ctx<'_>, node: &AstNode<'_>) -> bool {
    for ancestor in ctx.ancestors(node) {
        match ancestor.kind() {
            AstKind::CallExpression(_) | AstKind::NewExpression(_) => return true,
            AstKind::StaticMemberExpression(_)
            | AstKind::ComputedMemberExpression(_)
            | AstKind::ChainExpression(_)
            | AstKind::TSAsExpression(_)
            | AstKind::TSNonNullExpression(_) => {}
            _ => return false,
        }
    }
    false
}
