//! `js/assert-argument-order`

use super::util;
use crate::prelude::*;

pub struct AssertArgumentOrder;

pub static ASSERT_ARGUMENT_ORDER: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["tests", "confusing"],
    remediation_minutes: 2,
    description: r#"Flags equality assertions whose *actual* slot holds a literal while the *expected* slot holds
the computed value: `expect(3).toBe(items.length)`, `assert.equal('ready', job.state)`.

**Why:** the assertion passes or fails the same way when the two are swapped, but its failure
message does not: `expected 7 to be 3` names the computed value as the expectation and the
constant as what the code produced, which sends the reader looking for the bug in the wrong
place.

**Fix:** put the computed value in the actual slot: the argument of `expect(…)`, the first argument
of `assert.equal` and its relatives.

```js
expect(3).toBe(items.length);                // flagged
assert.strictEqual('ready', job.state);      // flagged

expect(items.length).toBe(3);                // compliant
assert.strictEqual(job.state, 'ready');      // compliant
```

Checked: `expect(…)` with `toBe`, `toEqual` and `toStrictEqual`, and `assert.equal`, `strictEqual`,
`deepEqual`, `deepStrictEqual`, `notEqual`, `notStrictEqual`, `notDeepEqual` and
`notDeepStrictEqual` (Node's `assert` and chai's `assert`, which both take the actual value
first). Literals are strings, numbers and templates without placeholders. Not reported: both
sides literal, `true`/`false`/`null`/`undefined` (other matchers fit better), a computed side
named like an expectation (`expected`, `expectedTotal`), and files that are not tests.
"#,
    ..RuleMeta::new(
        "js/assert-argument-order",
        "js",
        "Assertions should take the actual value in the actual slot",
    )
};

impl JsRule for AssertArgumentOrder {
    fn meta(&self) -> &'static RuleMeta {
        &ASSERT_ARGUMENT_ORDER
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CallExpression]
    }

    fn callee_names(&self) -> &'static [&'static str] {
        &[
            "toBe",
            "toEqual",
            "toStrictEqual",
            "equal",
            "strictEqual",
            "deepEqual",
            "deepStrictEqual",
            "notEqual",
            "notStrictEqual",
            "notDeepEqual",
            "notDeepStrictEqual",
        ]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.is_test()
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::CallExpression(call) = node.kind() else {
            return;
        };
        let Expression::StaticMemberExpression(callee) = helpers::unwrap_expr(&call.callee) else {
            return;
        };
        let method = callee.property.name.as_str();
        match method {
            "toBe" | "toEqual" | "toStrictEqual" => {
                let [expected] = call.arguments.as_slice() else {
                    return;
                };
                let Some(expected) = expected.as_expression() else {
                    return;
                };
                // `expect(actual)[.not].toBe(expected)`
                let mut object = helpers::unwrap_expr(&callee.object);
                if let Expression::StaticMemberExpression(m) = object {
                    if m.property.name != "not" {
                        return;
                    }
                    object = helpers::unwrap_expr(&m.object);
                }
                let Expression::CallExpression(expect) = object else {
                    return;
                };
                let Expression::Identifier(id) = helpers::unwrap_expr(&expect.callee) else {
                    return;
                };
                if id.name != "expect" || !util::is_test_api(ctx, id) {
                    return;
                }
                let [actual] = expect.arguments.as_slice() else {
                    return;
                };
                let Some(actual) = actual.as_expression() else {
                    return;
                };
                if swapped(actual, expected) {
                    ctx.emit(
                        actual.span(),
                        format!(
                            "Swap the arguments: pass the computed value to 'expect' and {} to '{method}'",
                            ctx.text(actual.span())
                        ),
                    );
                }
            }
            "equal" | "strictEqual" | "deepEqual" | "deepStrictEqual" | "notEqual"
            | "notStrictEqual" | "notDeepEqual" | "notDeepStrictEqual" => {
                let Expression::Identifier(object) = helpers::unwrap_expr(&callee.object) else {
                    return;
                };
                if object.name != "assert" || !is_assert_module(ctx, object) {
                    return;
                }
                let (Some(actual), Some(expected)) = (
                    call.arguments.first().and_then(Argument::as_expression),
                    call.arguments.get(1).and_then(Argument::as_expression),
                ) else {
                    return;
                };
                if swapped(actual, expected) {
                    ctx.emit(
                        actual.span(),
                        format!(
                            "Swap the first two arguments of 'assert.{method}': the computed value comes first, {} second",
                            ctx.text(actual.span())
                        ),
                    );
                }
            }
            _ => {}
        }
    }
}

/// A literal in the actual slot and a computed value in the expected slot.
fn swapped(actual: &Expression<'_>, expected: &Expression<'_>) -> bool {
    is_constant(actual)
        && !is_constant(expected)
        && !is_value_literal(expected)
        && !named_expected(expected)
}

/// Strings, numbers, bigints, templates without placeholders.
fn is_constant(expr: &Expression<'_>) -> bool {
    match helpers::unwrap_expr(expr) {
        Expression::StringLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BigIntLiteral(_) => true,
        Expression::TemplateLiteral(t) => t.expressions.is_empty(),
        e @ Expression::UnaryExpression(_) => helpers::number_value(e).is_some(),
        _ => false,
    }
}

/// Literals of any kind, and object or array literals: not a computed value either.
fn is_value_literal(expr: &Expression<'_>) -> bool {
    matches!(
        helpers::unwrap_expr(expr),
        Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_)
            | Expression::RegExpLiteral(_)
            | Expression::ObjectExpression(_)
            | Expression::ArrayExpression(_)
            | Expression::ArrowFunctionExpression(_)
            | Expression::FunctionExpression(_)
    ) || matches!(helpers::unwrap_expr(expr), Expression::Identifier(id) if id.name == "undefined")
}

/// `expected`, `expectedTotal`, `EXPECTED_ROWS`: the author calls it the expectation.
fn named_expected(expr: &Expression<'_>) -> bool {
    match helpers::unwrap_expr(expr) {
        Expression::Identifier(id) => id.name.to_ascii_lowercase().starts_with("expected"),
        Expression::StaticMemberExpression(m) => {
            m.property.name.to_ascii_lowercase().starts_with("expected")
        }
        _ => false,
    }
}

/// `assert` is the global, an import or `require('assert')`.
fn is_assert_module(ctx: &Ctx<'_>, object: &IdentifierReference<'_>) -> bool {
    match ctx.reference_symbol(object) {
        None => true,
        Some(symbol) => {
            ctx.symbol_flags(symbol).contains(SymbolFlags::Import)
                || matches!(ctx.symbol_declaration(symbol).kind(), AstKind::VariableDeclarator(d)
                    if matches!(d.init.as_ref().map(helpers::unwrap_expr),
                        Some(Expression::CallExpression(c)) if c.callee.is_specific_id("require")))
        }
    }
}
