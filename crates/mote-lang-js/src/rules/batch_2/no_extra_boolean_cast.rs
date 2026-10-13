//! `js/no-extra-boolean-cast`

use super::util;
use crate::prelude::*;

pub struct NoExtraBooleanCast;

pub static NO_EXTRA_BOOLEAN_CAST: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["redundant"],
    remediation_minutes: 1,
    description: r#"Flags `!!x` and `Boolean(x)` where the result is used in a place that already
converts its value to a boolean: the condition of `if`, `while`, `do … while`, `for`, or `?:`,
the operand of `!`, and the argument of `Boolean(…)`.

**Why:** those contexts test truthiness anyway, so the conversion is noise that makes the
condition longer and suggests the author was unsure how it works.

**Fix:** remove the double negation or the `Boolean` call. Keep them where a real boolean value
is needed, such as a return value, an assignment, or a property.

```js
if (!!user) {}              // flagged
while (Boolean(next)) {}    // flagged

if (user) {}                // compliant
const hasUser = !!user;     // compliant: stores a boolean
```

**Fix available.** `mote fix` removes the `!!` or the `Boolean(…)` call around the value (in
parentheses where the context needs them): converting to a boolean has no side effect, and the
context converts the value the same way.
"#,
    fixable: true,
    ..RuleMeta::new(
        "js/no-extra-boolean-cast",
        "js",
        "Redundant boolean conversions should be removed",
    )
};

/// The argument text of a one-argument call between `open` (the callee's end) and `close` (the
/// closing parenthesis): inside the call's parentheses, without a trailing comma.
fn inside_span(ctx: &Ctx<'_>, open: u32, close: u32) -> Span {
    let text = ctx.text(Span::new(open, close));
    let start = open + text.find('(').map_or(0, |i| i as u32 + 1);
    let body = ctx.text(Span::new(start, close));
    let trimmed = body.trim_end().trim_end_matches(',');
    Span::new(start, start + trimmed.len() as u32)
}

/// Whether `expr` can stand as the operand of `!` without parentheses.
fn is_operand_level(expr: &Expression<'_>) -> bool {
    matches!(
        expr,
        Expression::Identifier(_)
            | Expression::ThisExpression(_)
            | Expression::StaticMemberExpression(_)
            | Expression::ComputedMemberExpression(_)
            | Expression::PrivateFieldExpression(_)
            | Expression::CallExpression(_)
            | Expression::NewExpression(_)
            | Expression::ParenthesizedExpression(_)
            | Expression::UnaryExpression(_)
            | Expression::AwaitExpression(_)
            | Expression::StringLiteral(_)
            | Expression::NumericLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_)
            | Expression::TemplateLiteral(_)
            | Expression::ArrayExpression(_)
    )
}

fn is_not<'b, 'a>(expr: &'b Expression<'a>) -> Option<&'b Expression<'a>> {
    match helpers::unwrap_expr(expr) {
        Expression::UnaryExpression(u) if u.operator == UnaryOperator::LogicalNot => {
            Some(&u.argument)
        }
        _ => None,
    }
}

/// Is the expression spanning `span` (a child of `parent`) in a position that coerces to boolean?
fn in_boolean_context(ctx: &Ctx<'_>, parent: &AstNode<'_>, span: Span) -> bool {
    match parent.kind() {
        AstKind::IfStatement(s) => s.test.span() == span,
        AstKind::WhileStatement(s) => s.test.span() == span,
        AstKind::DoWhileStatement(s) => s.test.span() == span,
        AstKind::ForStatement(s) => s.test.as_ref().is_some_and(|t| t.span() == span),
        AstKind::ConditionalExpression(c) => c.test.span() == span,
        AstKind::UnaryExpression(u) => u.operator == UnaryOperator::LogicalNot,
        AstKind::CallExpression(c) => {
            ctx.is_global(&c.callee, "Boolean")
                && c.arguments.first().is_some_and(|a| a.span() == span)
        }
        AstKind::NewExpression(n) => {
            ctx.is_global(&n.callee, "Boolean")
                && n.arguments.first().is_some_and(|a| a.span() == span)
        }
        _ => false,
    }
}

impl JsRule for NoExtraBooleanCast {
    fn meta(&self) -> &'static RuleMeta {
        &NO_EXTRA_BOOLEAN_CAST
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::UnaryExpression, AstType::CallExpression]
    }

    fn callee_names(&self) -> &'static [&'static str] {
        &["Boolean"]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        // The text that replaces the node, and whether it needs parentheses in a tighter context.
        let mut replacement: Option<(Span, bool)> = None;
        let mut always_parens = false;
        let message = match node.kind() {
            AstKind::UnaryExpression(u) => {
                if u.operator != UnaryOperator::LogicalNot {
                    return;
                }
                let Some(inner) = is_not(&u.argument) else {
                    return;
                };
                // `!!!x`: the inner `!!` is reported (its parent `!` is the context).
                if is_not(inner).is_some() {
                    return;
                }
                // The operand of the inner `!` as written (its parentheses are not in the tree):
                // a valid operand of `!`, so valid wherever `!!operand` was.
                let not = helpers::unwrap_expr(&u.argument).span();
                replacement = Some((Span::new(not.start + 1, not.end), false));
                "Remove the redundant '!!': this context already converts the value to a boolean"
            }
            AstKind::CallExpression(call) => {
                if call.optional
                    || call.arguments.len() != 1
                    || matches!(call.arguments[0], Argument::SpreadElement(_))
                    || !ctx.is_global(&call.callee, "Boolean")
                {
                    return;
                }
                // The argument's own parentheses are not in the tree: take everything between
                // the call's.
                if let Some(arg) = call.arguments[0].as_expression() {
                    let open = call.callee.span().end;
                    let close = call.span.end.saturating_sub(1);
                    // Parentheses of their own around the argument make the text between the
                    // call's longer than the argument's span.
                    let inside = inside_span(ctx, open, close);
                    let parenthesized = ctx.text(inside).trim() != ctx.text(arg.span());
                    let span = if parenthesized { inside } else { arg.span() };
                    // An assignment keeps parentheses even as a condition: `if ((a = b))` says
                    // the assignment is meant.
                    let assignment = matches!(arg, Expression::AssignmentExpression(_));
                    replacement = Some((
                        span,
                        !parenthesized && (assignment || !is_operand_level(arg)),
                    ));
                    always_parens = assignment && !parenthesized;
                }
                "Remove the redundant 'Boolean' call: this context already converts the value to a boolean"
            }
            _ => return,
        };
        let Some((parent, span)) = util::parent_through_wrappers(ctx, node) else {
            return;
        };
        if in_boolean_context(ctx, parent, span) {
            let edit = replacement.map(|(inner, may_need_parens)| {
                let text = ctx.text(inner).trim();
                // A statement's condition or a `Boolean(…)` argument takes any expression; the
                // operand of `!` and the test of `?:` bind tighter.
                let loose = matches!(
                    parent.kind(),
                    AstKind::IfStatement(_)
                        | AstKind::WhileStatement(_)
                        | AstKind::DoWhileStatement(_)
                        | AstKind::ForStatement(_)
                        | AstKind::CallExpression(_)
                        | AstKind::NewExpression(_)
                ) && span == node.span();
                let new = if may_need_parens && (!loose || always_parens) {
                    format!("({text})")
                } else {
                    text.to_string()
                };
                Edit::replace(node.span().start as usize, node.span().end as usize, new)
            });
            let emitted = ctx.emit(node.span(), message);
            if let Some(edit) = edit {
                emitted.fix("Remove the redundant conversion", vec![edit]);
            }
        }
    }
}
