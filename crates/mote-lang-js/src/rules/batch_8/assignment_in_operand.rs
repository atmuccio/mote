//! `js/assignment-in-operand`

use crate::prelude::*;

pub struct AssignmentInOperand;

pub static ASSIGNMENT_IN_OPERAND: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["confusing", "side-effects"],
    remediation_minutes: 5,
    params: &[ParamSpec {
        name: "report_array_index",
        default: ParamDefault::Bool(false),
        description: "Also report `++`/`--` used directly as an array index (`buf[pos++] = b`).",
    }],
    description: r#"Flags increments, decrements and assignments buried inside a larger expression (a call
argument, an operand of another operator, an array or object literal, a template, a ternary
branch) instead of standing as statements of their own.

**Why:** an expression that both computes a value and changes a variable makes the reader track
two things at once, and the order of those effects is easy to misread (`items[i++]` against
`items[++i]`, `f(a = b)` against `f(a === b)`). Such code is also fragile to edit: moving or
duplicating the expression moves or duplicates the side effect.

**Fix:** perform the update in its own statement, before or after the expression.

```js
process(items[index], index++);          // flagged
total = base + (delta = compute());      // flagged

process(items[index], index);            // compliant
index++;
delta = compute();
total = base + delta;
```

Not reported: statements (`i++;`, `a = b, c = d;`), `for` loop headers, arrow functions whose body
is the assignment (`el => (this.ref = el)`, `() => count++`), the value of a `return`, a variable,
field or parameter initializer, an assignment's right side (`n = count++`, `a = b = 0`, left to
`js/no-multi-assign`), operands of `void` (`void (x = y)` discards the value on purpose),
assignments anywhere in the condition of an `if`, `while`, `do` or `for`
(left to `js/no-cond-assign`), and increments or decrements in a loop condition
(`while (n-- > 0)`). An increment or decrement inside an `if` condition (`if (++attempts > max)`)
is reported: whether the test sees the old or the new value is exactly what readers get wrong. By
default `++`/`--` used directly as an array index (`buf[pos++] = b`) are accepted as idiomatic;
set `report_array_index` to report them too. Off by default: short-circuit updates
(`ready && (done = true)`), id counters (`{ id: next++ }`) and reference counts
(`if (--refs === 0)`) are common JavaScript idioms, so the rule suits focused clean-ups.
"#,
    ..RuleMeta::new(
        "js/assignment-in-operand",
        "js",
        "Assignments and increments should not be nested in other expressions",
    )
};

impl JsRule for AssignmentInOperand {
    fn meta(&self) -> &'static RuleMeta {
        &ASSIGNMENT_IN_OPERAND
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::AssignmentExpression, AstType::UpdateExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let is_update = match node.kind() {
            AstKind::AssignmentExpression(_) => false,
            AstKind::UpdateExpression(_) => true,
            _ => return,
        };
        // How is the value used? Step through TypeScript wrappers and comma sequences.
        let mut child = node;
        let parent = loop {
            let Some(parent) = ctx.parent(child) else {
                return;
            };
            match parent.kind() {
                AstKind::TSAsExpression(_)
                | AstKind::TSSatisfiesExpression(_)
                | AstKind::TSNonNullExpression(_)
                | AstKind::TSTypeAssertion(_)
                | AstKind::ParenthesizedExpression(_)
                | AstKind::SequenceExpression(_) => child = parent,
                _ => break parent,
            }
        };
        let child_span = child.span();
        match parent.kind() {
            // Statements, loop headers, returns, initializers.
            AstKind::ExpressionStatement(_)
            | AstKind::ForStatement(_)
            | AstKind::ReturnStatement(_)
            | AstKind::VariableDeclarator(_)
            | AstKind::PropertyDefinition(_)
            | AstKind::AccessorProperty(_)
            | AstKind::AssignmentPattern(_)
            | AstKind::FormalParameter(_)
            | AstKind::ThrowStatement(_) => return,
            AstKind::AssignmentExpression(a) if a.right.span() == child_span => return,
            AstKind::ComputedMemberExpression(m)
                if is_update
                    && m.expression.span() == child_span
                    && !ctx.param_bool("report_array_index") =>
            {
                return;
            }
            AstKind::ArrowFunctionExpression(_) => return,
            // `void (x = y)`: the value is explicitly discarded.
            AstKind::UnaryExpression(u) if u.operator == UnaryOperator::Void => return,
            _ => {}
        }
        if in_condition(ctx, node, is_update) {
            return;
        }
        let span = node.span();
        let text = ctx.text(span);
        let message = if is_update {
            format!("Move '{text}' into a statement of its own")
        } else {
            "Move this assignment into a statement of its own".to_string()
        };
        let span = match text.find('\n') {
            Some(i) => Span::new(span.start, span.start + i as u32),
            None => span,
        };
        ctx.emit(span, message);
    }
}

/// Is the node inside the condition of an `if`, `while`, `do` or `for` (assignments), or of a
/// loop (increments and decrements), in the same function?
fn in_condition(ctx: &Ctx<'_>, node: &AstNode<'_>, is_update: bool) -> bool {
    let at = node.span();
    for ancestor in ctx.ancestors(node) {
        let test = match ancestor.kind() {
            AstKind::IfStatement(s) if !is_update => s.test.span(),
            AstKind::WhileStatement(s) => s.test.span(),
            AstKind::DoWhileStatement(s) => s.test.span(),
            // The whole header: initializer, condition and update.
            AstKind::ForStatement(s) => Span::new(s.span.start, s.body.span().start),
            AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) | AstKind::Class(_) => {
                return false;
            }
            kind if kind.is_statement() => return false,
            _ => continue,
        };
        return test.start <= at.start && at.end <= test.end;
    }
    false
}
