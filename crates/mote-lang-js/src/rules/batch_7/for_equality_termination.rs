//! `js/for-equality-termination`

use super::util::is_function_node;
use crate::prelude::*;
use crate::rules::batch_6::util::descendants;

pub struct ForEqualityTermination;

pub static FOR_EQUALITY_TERMINATION: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["suspicious"],
    remediation_minutes: 2,
    description: r#"Flags a `for` loop that stops when its counter becomes *equal* to the bound (`i != n`) while the
counter can jump over the bound: it is also changed inside the loop body, it moves by a step that
provably misses the bound, or it moves by a fraction.

**Why:** `i != n` only ends the loop when `i` lands exactly on `n`. A counter that is sometimes
advanced twice (skipping an escaped character, a pair of items) or that moves by 2 from an odd
distance steps over the bound and the loop runs on — reading past the end of the data, or never
ending. Fractional steps accumulate rounding errors and may never hit the bound at all.

**Fix:** use a relational test (`i < n`, `i > n`), which stops however the bound is crossed.

```js
for (let i = 0; i !== text.length; i++) {
  if (text[i] === '\\') i++;          // flagged: can skip past text.length
}
for (let i = 0; i != 9; i += 2) {}    // flagged: 0, 2, … 8, 10 never equals 9
for (let x = 0; x != 1; x += 0.1) {}  // flagged: rounding never lands on 1

for (let i = 0; i < text.length; i++) {
  if (text[i] === '\\') i++;          // compliant
}
for (let i = 0; i != n; i++) {}       // compliant: a step of 1 cannot skip the bound
```

Steps that are not literals (`i += step`) and loops whose counter is not updated arithmetically
(`node != null; node = node.next`) are not reported.
"#,
    ..RuleMeta::new(
        "js/for-equality-termination",
        "js",
        "For-loop conditions should not test for equality with a bound the counter can skip",
    )
};

enum Step {
    /// `++`, `--`, `+= 1`, `-= 1`.
    Unit,
    /// A literal step other than ±1 (signed).
    Literal(f64),
}

impl JsRule for ForEqualityTermination {
    fn meta(&self) -> &'static RuleMeta {
        &FOR_EQUALITY_TERMINATION
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
        let Expression::BinaryExpression(test) = helpers::unwrap_expr(test) else {
            return;
        };
        if !matches!(
            test.operator,
            BinaryOperator::Inequality | BinaryOperator::StrictInequality
        ) {
            return;
        }
        // The counter is the side the update moves.
        let sides = [(&test.left, &test.right), (&test.right, &test.left)];
        let Some((counter, bound, step)) = sides.iter().find_map(|(c, b)| {
            let Expression::Identifier(id) = helpers::unwrap_expr(c) else {
                return None;
            };
            step_of(update, id.name.as_str()).map(|s| (&**id, *b, s))
        }) else {
            return;
        };
        let Some(symbol) = ctx.reference_symbol(counter) else {
            return;
        };
        let name = counter.name.as_str();
        let reason = match step {
            Step::Unit => {
                if !changed_in_body(ctx, node, &stmt.body, symbol) {
                    return;
                }
                "the counter is also changed inside the loop and can jump over the bound"
                    .to_string()
            }
            Step::Literal(step) => {
                if step.fract() != 0.0 {
                    format!("a fractional step ({step}) accumulates rounding errors and may never land on the bound")
                } else {
                    let (Some(start), Some(end)) = (
                        initial_value(stmt, name),
                        helpers::number_value(helpers::unwrap_expr(bound)),
                    ) else {
                        return;
                    };
                    let distance = end - start;
                    if distance.fract() == 0.0 && distance % step == 0.0 && distance / step >= 0.0 {
                        return;
                    }
                    format!("counting from {start} by {step} never lands on {end}")
                }
            }
        };
        ctx.emit(
            test.span,
            format!(
                "Use '<' or '>' instead of '{}' to end this loop: {reason}",
                test.operator.as_str()
            ),
        );
    }
}

/// How the update expression moves `name`, if it is an arithmetic step on it.
fn step_of(update: &Expression<'_>, name: &str) -> Option<Step> {
    match helpers::unwrap_expr(update) {
        Expression::SequenceExpression(seq) => seq.expressions.iter().find_map(|e| step_of(e, name)),
        Expression::UpdateExpression(u) => {
            matches!(&u.argument, SimpleAssignmentTarget::AssignmentTargetIdentifier(id) if id.name == name)
                .then_some(Step::Unit)
        }
        Expression::AssignmentExpression(a) => {
            let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else {
                return None;
            };
            if id.name != name {
                return None;
            }
            let (sign, amount) = match a.operator {
                AssignmentOperator::Addition => (1.0, helpers::number_value(helpers::unwrap_expr(&a.right))),
                AssignmentOperator::Subtraction => (-1.0, helpers::number_value(helpers::unwrap_expr(&a.right))),
                AssignmentOperator::Assign => match helpers::unwrap_expr(&a.right) {
                    // `i = i + 2`, `i = 2 + i`, `i = i - 2`
                    Expression::BinaryExpression(b) => {
                        let is_counter = |e: &Expression<'_>| {
                            matches!(helpers::unwrap_expr(e), Expression::Identifier(r) if r.name == name)
                        };
                        match b.operator {
                            BinaryOperator::Addition if is_counter(&b.left) => {
                                (1.0, helpers::number_value(helpers::unwrap_expr(&b.right)))
                            }
                            BinaryOperator::Addition if is_counter(&b.right) => {
                                (1.0, helpers::number_value(helpers::unwrap_expr(&b.left)))
                            }
                            BinaryOperator::Subtraction if is_counter(&b.left) => {
                                (-1.0, helpers::number_value(helpers::unwrap_expr(&b.right)))
                            }
                            _ => return None,
                        }
                    }
                    _ => return None,
                },
                _ => return None,
            };
            let amount = amount?;
            if !amount.is_finite() || amount == 0.0 {
                return None;
            }
            if amount.abs() == 1.0 {
                Some(Step::Unit)
            } else {
                Some(Step::Literal(sign * amount))
            }
        }
        _ => None,
    }
}

/// The literal the loop's `init` gives the counter (`let i = 0`, `i = 0`).
fn initial_value(stmt: &ForStatement<'_>, name: &str) -> Option<f64> {
    match stmt.init.as_ref()? {
        ForStatementInit::VariableDeclaration(decl) => {
            decl.declarations.iter().find_map(|d| match &d.id {
                BindingPattern::BindingIdentifier(id) if id.name == name => {
                    helpers::number_value(helpers::unwrap_expr(d.init.as_ref()?))
                }
                _ => None,
            })
        }
        init => {
            let expr = init.as_expression()?;
            let Expression::AssignmentExpression(a) = helpers::unwrap_expr(expr) else {
                return None;
            };
            match &a.left {
                AssignmentTarget::AssignmentTargetIdentifier(id)
                    if id.name == name && a.operator == AssignmentOperator::Assign =>
                {
                    helpers::number_value(helpers::unwrap_expr(&a.right))
                }
                _ => None,
            }
        }
    }
}

/// Is the counter incremented or decremented (`i++`, `i += k`) inside the loop body, outside
/// nested functions?
fn changed_in_body(
    ctx: &Ctx<'_>,
    loop_node: &AstNode<'_>,
    body: &Statement<'_>,
    symbol: SymbolId,
) -> bool {
    let body_span = body.span();
    let mut skip_until = 0;
    for inner in descendants(ctx, loop_node) {
        let span = inner.span();
        if span.start < body_span.start || span.start < skip_until {
            continue;
        }
        let kind = inner.kind();
        if is_function_node(kind) {
            skip_until = span.end;
            continue;
        }
        let target = match kind {
            AstKind::UpdateExpression(u) => match &u.argument {
                SimpleAssignmentTarget::AssignmentTargetIdentifier(id) => Some(&**id),
                _ => None,
            },
            AstKind::AssignmentExpression(a)
                if matches!(
                    a.operator,
                    AssignmentOperator::Addition | AssignmentOperator::Subtraction
                ) =>
            {
                match &a.left {
                    AssignmentTarget::AssignmentTargetIdentifier(id) => Some(&**id),
                    _ => None,
                }
            }
            _ => None,
        };
        if target.is_some_and(|id| ctx.reference_symbol(id) == Some(symbol)) {
            return true;
        }
    }
    false
}
