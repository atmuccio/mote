//! `js/no-unused-expressions`

use crate::prelude::*;

pub struct NoUnusedExpressions;

pub static NO_UNUSED_EXPRESSIONS: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["suspicious", "unused"],
    remediation_minutes: 5,
    params: &[ParamSpec {
        name: "allow_short_circuit",
        default: ParamDefault::Bool(true),
        description: "Accept short-circuit statements such as `ready && start()`; set to false to report them too.",
    }],
    description: r#"Flags expression statements that compute a value and throw it away without any possible
effect: `a === b;`, `x;`, `total + tax;`, `'text';` outside the directive prologue, `[a, b];`.

**Why:** such a statement changes nothing, so it is almost always a mistake — a missing
assignment (`total = total + tax`), a missing `return`, or `==` typed for `=`.

**Fix:** assign or return the value, or delete the statement. To evaluate something only for its
side effects, write `void expr`.

```js
count + 1;                     // flagged: the result is discarded
isReady === true;              // flagged: a comparison with no effect
count += 1;                    // compliant
ready && start();              // compliant: short-circuit call
isValid ? accept() : reject(); // compliant
void promise;                  // compliant: explicitly discarded
```

Only statements that cannot have any effect are reported: nothing in them calls code (calls,
`new`, tagged templates, spreads, class definitions), writes (assignments, `++`/`--`, `delete`),
or suspends (`await`, `yield`). A statement that is just a property read (`el.offsetHeight;`,
`expect(x).to.be.true;`) is accepted because getters are sometimes read for their effect, and so
are JSX elements, directives (`'use strict'`), a file consisting of a single function, object, or
array expression (a snippet evaluated by a tool), and TypeScript type tests written as
`value satisfies Type;`.

Setting the `allow_short_circuit` parameter to `false` also reports short-circuit statements such
as `ready && start();` and `cache || load();`, for codebases that prefer an explicit `if`.
"#,
    ..RuleMeta::new(
        "js/no-unused-expressions",
        "js",
        "Expressions should not be evaluated only to be discarded",
    )
};

impl JsRule for NoUnusedExpressions {
    fn meta(&self) -> &'static RuleMeta {
        &NO_UNUSED_EXPRESSIONS
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::ExpressionStatement]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::ExpressionStatement(stmt) = node.kind() else {
            return;
        };
        // `value satisfies Shape;` is a compile-time type test: the statement exists for the
        // type checker.
        if is_type_test(&stmt.expression) {
            return;
        }
        let expr = helpers::unwrap_expr(&stmt.expression);
        // A bare property read may run a getter on purpose (lazy initialization, forcing layout,
        // assertion chains such as `expect(x).to.be.true`).
        // With `allow_short_circuit` off, `ready && start();` is reported although it has an effect.
        let short_circuit = matches!(expr, Expression::LogicalExpression(_))
            && may_have_effect(expr)
            && !ctx.param_bool("allow_short_circuit");
        if !short_circuit && (is_member_read(expr) || may_have_effect(expr)) {
            return;
        }
        let parent = ctx.parent(node);
        match parent.map(AstNode::kind) {
            // The expression body of `() => value` is represented as an expression statement.
            Some(AstKind::FunctionBody(_)) => {
                if matches!(parent.and_then(|p| ctx.parent_kind(p)), Some(AstKind::ArrowFunctionExpression(a)) if a.expression)
                {
                    return;
                }
            }
            // A file that is a single function, object, or array expression is a snippet evaluated by
            // some tool (`async page => { … }`), not a module.
            Some(AstKind::Program(program))
                if program.body.len() == 1
                    && matches!(
                        expr,
                        Expression::FunctionExpression(_)
                            | Expression::ArrowFunctionExpression(_)
                            | Expression::ObjectExpression(_)
                            | Expression::ArrayExpression(_)
                    ) =>
            {
                return;
            }
            _ => {}
        }
        let span = stmt.expression.span();
        let first_line_end = ctx
            .text(span)
            .find('\n')
            .map_or(span.end, |i| span.start + i as u32);
        let message = if short_circuit {
            "Use an 'if' statement instead of a short-circuit expression used as a statement"
        } else {
            "Use or remove this expression: its value is computed and discarded"
        };
        ctx.emit(Span::new(span.start, first_line_end), message);
    }
}

/// A `satisfies` expression, possibly under other type-level wrappers (`x satisfies T as U`).
fn is_type_test(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::TSSatisfiesExpression(_) => true,
        Expression::TSAsExpression(e) => is_type_test(&e.expression),
        Expression::TSNonNullExpression(e) => is_type_test(&e.expression),
        Expression::TSTypeAssertion(e) => is_type_test(&e.expression),
        _ => false,
    }
}

fn is_member_read(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::StaticMemberExpression(_)
        | Expression::ComputedMemberExpression(_)
        | Expression::PrivateFieldExpression(_) => true,
        Expression::ChainExpression(c) => !matches!(c.expression, ChainElement::CallExpression(_)),
        _ => false,
    }
}

/// Could evaluating the expression do anything besides producing a value? Anything that calls
/// code (calls, `new`, tagged templates, spreads, class definitions), writes (assignments,
/// updates, `delete`), suspends (`await`, `yield`), or is explicitly discarded (`void`) counts.
/// Property reads nested in a larger expression are treated as plain reads.
fn may_have_effect(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::CallExpression(_)
        | Expression::NewExpression(_)
        | Expression::AssignmentExpression(_)
        | Expression::UpdateExpression(_)
        | Expression::AwaitExpression(_)
        | Expression::YieldExpression(_)
        | Expression::ImportExpression(_)
        | Expression::TaggedTemplateExpression(_)
        | Expression::V8IntrinsicExpression(_)
        | Expression::ClassExpression(_)
        // JSX statements are left to JSX-specific tooling.
        | Expression::JSXElement(_)
        | Expression::JSXFragment(_)
        | Expression::ParenthesizedExpression(_)
        | Expression::Super(_) => true,
        Expression::UnaryExpression(u) => {
            matches!(u.operator, UnaryOperator::Delete | UnaryOperator::Void)
                || may_have_effect(&u.argument)
        }
        Expression::BinaryExpression(b) => may_have_effect(&b.left) || may_have_effect(&b.right),
        Expression::PrivateInExpression(p) => may_have_effect(&p.right),
        Expression::LogicalExpression(l) => may_have_effect(&l.left) || may_have_effect(&l.right),
        Expression::ConditionalExpression(c) => {
            may_have_effect(&c.test) || may_have_effect(&c.consequent) || may_have_effect(&c.alternate)
        }
        Expression::SequenceExpression(s) => s.expressions.iter().any(may_have_effect),
        Expression::TemplateLiteral(t) => t.expressions.iter().any(may_have_effect),
        Expression::ArrayExpression(a) => a.elements.iter().any(|e| match e {
            ArrayExpressionElement::SpreadElement(_) => true,
            ArrayExpressionElement::Elision(_) => false,
            _ => e.as_expression().is_some_and(may_have_effect),
        }),
        Expression::ObjectExpression(o) => o.properties.iter().any(|p| match p {
            ObjectPropertyKind::SpreadProperty(_) => true,
            ObjectPropertyKind::ObjectProperty(p) => {
                may_have_effect(&p.value)
                    || (p.computed && p.key.as_expression().is_some_and(may_have_effect))
            }
        }),
        Expression::StaticMemberExpression(m) => may_have_effect(&m.object),
        Expression::PrivateFieldExpression(m) => may_have_effect(&m.object),
        Expression::ComputedMemberExpression(m) => {
            may_have_effect(&m.object) || may_have_effect(&m.expression)
        }
        Expression::ChainExpression(c) => match &c.expression {
            ChainElement::CallExpression(_) => true,
            ChainElement::TSNonNullExpression(e) => may_have_effect(&e.expression),
            ChainElement::StaticMemberExpression(m) => may_have_effect(&m.object),
            ChainElement::PrivateFieldExpression(m) => may_have_effect(&m.object),
            ChainElement::ComputedMemberExpression(m) => {
                may_have_effect(&m.object) || may_have_effect(&m.expression)
            }
        },
        Expression::TSAsExpression(e) => may_have_effect(&e.expression),
        Expression::TSSatisfiesExpression(e) => may_have_effect(&e.expression),
        Expression::TSNonNullExpression(e) => may_have_effect(&e.expression),
        Expression::TSTypeAssertion(e) => may_have_effect(&e.expression),
        Expression::TSInstantiationExpression(e) => may_have_effect(&e.expression),
        _ => false,
    }
}
