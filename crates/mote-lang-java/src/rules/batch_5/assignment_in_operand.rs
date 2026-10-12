//! `java/assignment-in-operand` — `foo(count++)`, `a + (b = c)`.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["side-effects", "readability"],
    remediation_minutes: 2,
    description: r#"Detects increments, decrements, and assignments buried inside a larger
expression — a method argument, an operand of another operator, an array index, a ternary
branch — instead of standing as statements of their own.

**Why it matters.** An expression that both computes a value and changes a variable makes
the reader track two things at once, and the order of those effects is easy to misread
(`items.get(i++)` vs `items.get(++i)`). Such code is also fragile to edit: moving or
duplicating the expression duplicates the side effect.

**How to fix.** Perform the update in its own statement, before or after the expression.

```java
// Noncompliant
process(items.get(index++));
total = base + (delta = compute());

// Compliant
process(items.get(index));
index++;
delta = compute();
total = base + delta;
```

Not reported: statements (`i++;`, `a = b = 0;`), `for` loop headers, expression lambdas
(`() -> count++`), the value of a `return`, a variable initializer, or an assignment's right
side (`int n = count++`), assignments inside the condition of an `if`, `while`, `do`, or
`for` (the `while ((line = r.readLine()) != null)` idiom; see
`java/assignment-in-condition`), and increments or decrements in a loop condition
(`while (n-- > 0)`). An increment or decrement inside an `if` condition
(`if (++attempts > MAX)`, `if (count++ > 0)`) is reported: whether the test sees the old or
the new value is exactly what readers get wrong. By default `++`/`--` used directly as an array index
(`buf[pos++] = b`) are accepted as idiomatic; set `report_array_index` to report them too."#,
    params: &[ParamSpec {
        name: "report_array_index",
        default: ParamDefault::Bool(false),
        description: "Also report `++`/`--` used directly as an array index (`buf[pos++]`).",
    }],
    ..RuleMeta::new(
        "java/assignment-in-operand",
        "java",
        "Assignment or increment inside an expression",
    )
};

pub struct AssignmentInOperand;

/// The kind of the `if` / `while` / `do` / `for` statement whose `condition` holds `node`
/// (below the nearest statement), if any.
fn condition_owner<'t>(ctx: &Ctx<'t>, node: Node<'t>) -> Option<&'static str> {
    let mut child = node;
    for a in ctx.ancestors(node) {
        match kind(a) {
            k @ ("if_statement" | "while_statement" | "do_statement" | "for_statement") => {
                return util::fld(a, F.condition)
                    .is_some_and(|c| c.id() == child.id())
                    .then_some(k);
            }
            "lambda_expression"
            | "class_body"
            | "block"
            | "expression_statement"
            | "local_variable_declaration"
            | "return_statement" => return None,
            _ => {}
        }
        child = a;
    }
    None
}

impl<'t> JavaRule<'t> for AssignmentInOperand {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["update_expression", "assignment_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some((parent, child)) = util::parent_skipping_parens(ctx, node) else {
            return;
        };
        let is_field = |f: u16| util::fld(parent, f).is_some_and(|c| c.id() == child.id());
        match kind(parent) {
            // Statements, `for` headers, expression lambdas, initializers.
            "expression_statement"
            | "lambda_expression"
            | "return_statement"
            | "yield_statement"
            | "throw_statement"
            | "variable_declarator"
            | "for_statement" => return,
            "assignment_expression" => {
                if is_field(F.right) {
                    return;
                }
            }
            "array_access" => {
                if kind(node) == "update_expression"
                    && is_field(F.index)
                    && !ctx.param_bool("report_array_index")
                {
                    return;
                }
            }
            // Statement-level uses that are not expressions of their own.
            "switch_rule" | "switch_label" => return,
            _ => {}
        }
        match condition_owner(ctx, node) {
            // `if (++count > LIMIT)`: an update hidden in a test is reported; assignments in
            // conditions (`if ((x = next()) != null)`) and updates in loop conditions
            // (`while (n-- > 0)`) are accepted idioms.
            Some("if_statement") if kind(node) == "update_expression" => {}
            Some(_) => return,
            None => {}
        }
        let (at, msg) = if kind(node) == "update_expression" {
            let op = if syntax::children(node).any(|c| kind(c) == "++") {
                "increment"
            } else {
                "decrement"
            };
            (
                node,
                format!("Move this {op} out of the enclosing expression into its own statement"),
            )
        } else {
            (
                syntax::operator(node).unwrap_or(node),
                "Move this assignment out of the enclosing expression into its own statement"
                    .to_string(),
            )
        };
        ctx.emit(at, msg);
    }
}
