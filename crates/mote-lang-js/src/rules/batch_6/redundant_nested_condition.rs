//! `js/redundant-nested-condition`

use super::util;
use crate::prelude::*;
use oxc_span::ContentEq;

pub struct RedundantNestedCondition;

pub static REDUNDANT_NESTED_CONDITION: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["suspicious", "redundant"],
    remediation_minutes: 5,
    description: r#"Flags an `if` nested directly in another `if` that tests a condition the outer one already decided:
the same condition again inside the `then` branch (always true), its negation there (always false),
or the same condition inside the `else` branch (always false). A part of an outer `a && b`
tested again in the `then` branch counts too.

**Why:** the inner test cannot change the outcome, so either it is dead weight, or — more often —
it was meant to test something else (a copy-paste slip, a variable renamed in one place only), and
the intended check is missing.

**Fix:** test the intended condition, or remove the redundant `if`.

```js
if (user.isAdmin) {
  if (user.isAdmin) grant();       // flagged: always true here
}
if (isOpen) {
  close();
} else {
  if (isOpen) retry();             // flagged: always false here
}

if (user.isAdmin) {
  if (user.isActive) grant();      // compliant
}
```

Only conditions without side effects (names, property reads, literals, comparisons, `!`, `&&`,
`||`) are compared, and only when nothing between the two tests can change them: the inner `if` is
the first statement of the branch, or follows statements that contain no calls, assignments, or
`await`.
"#,
    ..RuleMeta::new(
        "js/redundant-nested-condition",
        "js",
        "Nested conditions should not repeat a test the enclosing if already decided",
    )
};

impl JsRule for RedundantNestedCondition {
    fn meta(&self) -> &'static RuleMeta {
        &REDUNDANT_NESTED_CONDITION
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::IfStatement]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::IfStatement(outer) = node.kind() else {
            return;
        };
        let test = helpers::unwrap_expr(&outer.test);
        if !is_pure(test) {
            return;
        }
        check_branch(ctx, node, test, &outer.consequent, true);
        if let Some(alternate) = &outer.alternate {
            // `else if` chains are covered by `js/no-dupe-else-if`.
            if !matches!(alternate, Statement::IfStatement(_)) {
                check_branch(ctx, node, test, alternate, false);
            }
        }
    }
}

fn check_branch<'a>(
    ctx: &mut Ctx<'a>,
    node: &AstNode<'a>,
    outer: &Expression<'a>,
    branch: &'a Statement<'a>,
    holds: bool,
) {
    let list = match branch {
        Statement::BlockStatement(b) => &b.body[..],
        other => std::slice::from_ref(other),
    };
    let Some(first) = list.first() else {
        return;
    };
    for statement in list {
        let Statement::IfStatement(inner) = statement else {
            continue;
        };
        let test = helpers::unwrap_expr(&inner.test);
        let Some(value) = decided(outer, test, holds) else {
            continue;
        };
        // Nothing before the inner `if` in the branch may change what the condition reads.
        if !quiet(
            ctx,
            node,
            Span::new(first.span().start, statement.span().start),
        ) {
            return;
        }
        ctx.emit(
            util::first_line(ctx, inner.test.span()),
            format!(
                "This condition is always {value} here: the enclosing 'if' already decided it; test the intended condition or remove the check"
            ),
        );
        return;
    }
}

/// The value of `inner` where `outer` is known to be `holds`, when it is decided.
fn decided(outer: &Expression<'_>, inner: &Expression<'_>, holds: bool) -> Option<bool> {
    if inner.content_eq(outer) {
        return Some(holds);
    }
    if let Expression::UnaryExpression(u) = inner {
        if u.operator == UnaryOperator::LogicalNot
            && helpers::unwrap_expr(&u.argument).content_eq(outer)
        {
            return Some(!holds);
        }
    }
    // `if (a && b) { if (a) … }`
    if holds {
        let mut conjuncts = Vec::new();
        collect_conjuncts(outer, &mut conjuncts);
        if conjuncts.len() > 1 && conjuncts.iter().any(|c| c.content_eq(inner)) {
            return Some(true);
        }
    }
    None
}

fn collect_conjuncts<'b, 'a>(expr: &'b Expression<'a>, out: &mut Vec<&'b Expression<'a>>) {
    match helpers::unwrap_expr(expr) {
        Expression::LogicalExpression(l) if l.operator == LogicalOperator::And => {
            collect_conjuncts(&l.left, out);
            collect_conjuncts(&l.right, out);
        }
        other => out.push(other),
    }
}

/// A condition that evaluates the same way twice when nothing runs in between.
fn is_pure(expr: &Expression<'_>) -> bool {
    match helpers::unwrap_expr(expr) {
        Expression::Identifier(_)
        | Expression::ThisExpression(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::StringLiteral(_) => true,
        Expression::StaticMemberExpression(m) => is_pure(&m.object),
        Expression::PrivateFieldExpression(m) => is_pure(&m.object),
        Expression::ComputedMemberExpression(m) => is_pure(&m.object) && is_pure(&m.expression),
        Expression::UnaryExpression(u) => {
            matches!(
                u.operator,
                UnaryOperator::LogicalNot | UnaryOperator::Typeof | UnaryOperator::UnaryNegation
            ) && is_pure(&u.argument)
        }
        Expression::BinaryExpression(b) => {
            !matches!(b.operator, BinaryOperator::In | BinaryOperator::Instanceof)
                && is_pure(&b.left)
                && is_pure(&b.right)
        }
        Expression::LogicalExpression(l) => is_pure(&l.left) && is_pure(&l.right),
        Expression::ChainExpression(c) => match &c.expression {
            ChainElement::StaticMemberExpression(m) => is_pure(&m.object),
            _ => false,
        },
        _ => false,
    }
}

/// Is the code of the `if` statement `node` inside `region` unable to change what a pure
/// condition reads: no calls, `new`, assignments, updates, `delete`, `await`, `yield`, tagged
/// templates, or nested functions?
fn quiet(ctx: &Ctx<'_>, node: &AstNode<'_>, region: Span) -> bool {
    util::descendants(ctx, node)
        // Descendants come in source order: none after the region is in it (scanning the rest
        // of every statement made nested `if`s quadratic).
        .take_while(|n| n.span().start < region.end)
        .filter(|n| util::within(n.span(), region))
        .all(|n| {
        !matches!(
            n.kind(),
            AstKind::CallExpression(_)
                | AstKind::NewExpression(_)
                | AstKind::AssignmentExpression(_)
                | AstKind::UpdateExpression(_)
                | AstKind::AwaitExpression(_)
                | AstKind::YieldExpression(_)
                | AstKind::TaggedTemplateExpression(_)
                | AstKind::Function(_)
                | AstKind::ArrowFunctionExpression(_)
                | AstKind::Class(_)
        ) && !matches!(n.kind(), AstKind::UnaryExpression(u) if u.operator == UnaryOperator::Delete)
    })
}
