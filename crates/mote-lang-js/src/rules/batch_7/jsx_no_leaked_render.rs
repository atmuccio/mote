//! `js/jsx-no-leaked-render`

use crate::prelude::*;
use crate::rules::batch_6::util::is_react_file;

pub struct JsxNoLeakedRender;

pub static JSX_NO_LEAKED_RENDER: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["react", "suspicious"],
    remediation_minutes: 2,
    description: r#"Flags conditional rendering with `&&` whose condition is a number: `{items.length && <List />}`,
`{count && <Badge />}` where `count` is declared as a number.

**Why:** `a && b` evaluates to `a` when `a` is falsy. For a number that is `0` (or `NaN`), and
unlike `false`, `null` or `undefined`, a number *is* rendered: the page shows a stray "0" where
nothing was meant to appear (in React Native, rendering it outside `<Text>` crashes the app).

**Fix:** make the condition a boolean: `items.length > 0 && …`, `!!count && …`, or use a
conditional expression `count ? <Badge /> : null`.

```jsx
{items.length && <List items={items} />}      // flagged: renders 0 for an empty list
{items.length > 0 && <List items={items} />}  // compliant
{isOpen && <Dialog />}                        // compliant: not a number
```

Operands count as numbers when they visibly are: a `.length` read, a numeric literal, arithmetic
(`a - b`, `a * b`, …), `Number(…)` / `parseInt(…)` / `parseFloat(…)`, or a variable or parameter
declared with the type `number`. Only `&&` expressions rendered as JSX children or returned from a
function are checked, not attribute values, and only in files that use React or Preact (other JSX
renderers, such as prompt builders, may not print numbers).
"#,
    ..RuleMeta::new(
        "js/jsx-no-leaked-render",
        "js",
        "Numbers should not be used as '&&' conditions in rendered JSX",
    )
};

impl JsRule for JsxNoLeakedRender {
    fn meta(&self) -> &'static RuleMeta {
        &JSX_NO_LEAKED_RENDER
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::LogicalExpression]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.is_jsx() && ctx.file_contains(AstType::JSXElement) && is_react_file(ctx)
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::LogicalExpression(expr) = node.kind() else {
            return;
        };
        if expr.operator != LogicalOperator::And {
            return;
        }
        // Only the outermost `&&` of a chain, rendered as a child or returned.
        let Some(parent) = ctx.outer_parent(node) else {
            return;
        };
        let rendered = match parent.kind() {
            AstKind::JSXExpressionContainer(_) => {
                matches!(
                    ctx.parent_kind(parent),
                    Some(AstKind::JSXElement(_) | AstKind::JSXFragment(_))
                )
            }
            AstKind::ReturnStatement(_) => true,
            AstKind::ExpressionStatement(_) => {
                crate::rules::batch_6::util::is_arrow_expression_body(ctx, parent)
            }
            _ => false,
        };
        if !rendered || !renders_jsx(&expr.right) {
            return;
        }
        // Every operand but the last decides whether the chain stops early.
        let mut operands = Vec::new();
        collect_conditions(&expr.left, &mut operands);
        for operand in operands {
            if let Some(what) = numeric(ctx, operand) {
                ctx.emit(
                    operand.span(),
                    format!("{what} is a number: when it is 0 this renders '0'; compare it ('> 0') or convert it to a boolean"),
                );
                return;
            }
        }
    }
}

/// The right-hand side renders markup: a JSX element or fragment (possibly in a conditional).
fn renders_jsx(expr: &Expression<'_>) -> bool {
    match helpers::unwrap_expr(expr) {
        Expression::JSXElement(_) | Expression::JSXFragment(_) => true,
        Expression::ConditionalExpression(c) => renders_jsx(&c.consequent) || renders_jsx(&c.alternate),
        Expression::LogicalExpression(l) => renders_jsx(&l.right),
        // `items.map((item) => <Row … />)`
        Expression::CallExpression(call) => {
            matches!(helpers::unwrap_expr(&call.callee), Expression::StaticMemberExpression(m) if m.property.name == "map")
                && call.arguments.first().and_then(Argument::as_expression).is_some_and(|f| {
                    match helpers::unwrap_expr(f) {
                        Expression::ArrowFunctionExpression(a) => {
                            a.expression
                                && a.body.statements.first().is_some_and(|s| {
                                    matches!(s, Statement::ExpressionStatement(e) if renders_jsx(&e.expression))
                                })
                        }
                        _ => false,
                    }
                })
        }
        _ => false,
    }
}

/// The operands of a left-nested `a && b && c` chain (all of them, in order).
fn collect_conditions<'b, 'a>(expr: &'b Expression<'a>, out: &mut Vec<&'b Expression<'a>>) {
    match helpers::unwrap_expr(expr) {
        Expression::LogicalExpression(l) if l.operator == LogicalOperator::And => {
            collect_conditions(&l.left, out);
            collect_conditions(&l.right, out);
        }
        other => out.push(other),
    }
}

/// Describes the operand when it is visibly a number.
fn numeric(ctx: &Ctx<'_>, expr: &Expression<'_>) -> Option<String> {
    match helpers::unwrap_expr(expr) {
        Expression::StaticMemberExpression(m) if m.property.name == "length" => {
            Some(format!("'{}'", excerpt(ctx.text(m.span))))
        }
        Expression::NumericLiteral(_) => Some("This literal".to_string()),
        Expression::BinaryExpression(b)
            if matches!(
                b.operator,
                BinaryOperator::Subtraction
                    | BinaryOperator::Multiplication
                    | BinaryOperator::Division
                    | BinaryOperator::Remainder
                    | BinaryOperator::Exponential
            ) =>
        {
            Some("This arithmetic result".to_string())
        }
        Expression::UnaryExpression(u)
            if matches!(
                u.operator,
                UnaryOperator::UnaryNegation | UnaryOperator::UnaryPlus
            ) =>
        {
            Some("This arithmetic result".to_string())
        }
        Expression::CallExpression(call) => {
            let is_number_call = ctx.is_global(&call.callee, "Number")
                || ctx.is_global(&call.callee, "parseInt")
                || ctx.is_global(&call.callee, "parseFloat")
                || matches!(
                    helpers::static_member_names(&call.callee),
                    Some(("Number", "parseInt" | "parseFloat"))
                );
            is_number_call.then(|| format!("'{}'", excerpt(ctx.text(call.span))))
        }
        Expression::Identifier(id) => {
            let symbol = ctx.reference_symbol(id)?;
            let annotation = match ctx.symbol_declaration(symbol).kind() {
                AstKind::VariableDeclarator(d) => d.type_annotation.as_ref(),
                AstKind::FormalParameter(p) => p.type_annotation.as_ref(),
                _ => None,
            }?;
            matches!(annotation.type_annotation, TSType::TSNumberKeyword(_))
                .then(|| format!("'{}'", id.name))
        }
        _ => None,
    }
}

fn excerpt(text: &str) -> String {
    let mut chars = text.chars();
    let short: String = chars.by_ref().take(40).collect();
    if chars.next().is_some() {
        format!("{short}…")
    } else {
        short
    }
}
