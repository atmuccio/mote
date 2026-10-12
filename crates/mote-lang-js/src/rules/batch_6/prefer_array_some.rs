//! `js/prefer-array-some`

use crate::prelude::*;

pub struct PreferArraySome;

pub static PREFER_ARRAY_SOME: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["array", "confusing"],
    remediation_minutes: 2,
    description: r#"Flags `list.find(…)` (or `findLast`) whose result is only used as a condition:
`if (users.find((u) => u.admin))`, `!items.find(…)`, `Boolean(list.find(…))`, or a local
`const found = list.find(…)` that is only ever tested (`if (!found) throw …`).

**Why:** `find` returns the matching element, not whether there is one. Used as a condition, the
answer is wrong whenever the match itself is falsy — `0`, `''`, `false`, `null` — and readers
have to work out that only the existence of a match matters. `some` returns `true` or `false` and
says exactly that.

**Fix:** use `some` with the same callback.

```js
if (users.find((u) => u.admin)) grant();    // flagged
if (users.some((u) => u.admin)) grant();    // compliant

const found = users.find((u) => u.admin);   // flagged: only tested below
if (!found) throw new Error('no admin');

const admin = users.find((u) => u.admin);   // compliant: the element is used
if (admin) grant(admin);
```

Only calls whose first argument is a function literal are checked (a `find(selector)` of a DOM or
query library is something else), and only where the result decides a branch: the test of `if`,
loops, and `? :`, the operand of `!`, `Boolean(…)`, and `&&` / `||` operands in those positions.
Comparisons such as `list.find(…) !== undefined` are left alone. A local is checked only when it
is a `const` or `let` declared inside a function, never reassigned, and used at least once, with
every use in one of those positions.
"#,
    ..RuleMeta::new(
        "js/prefer-array-some",
        "js",
        "Use some() instead of find() to test whether an element matches",
    )
};

impl JsRule for PreferArraySome {
    fn meta(&self) -> &'static RuleMeta {
        &PREFER_ARRAY_SOME
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CallExpression]
    }

    fn callee_names(&self) -> &'static [&'static str] {
        &["find", "findLast"]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::CallExpression(call) = node.kind() else {
            return;
        };
        let Some(member) = call.callee.as_member_expression() else {
            return;
        };
        let Some(name) = member.static_property_name() else {
            return;
        };
        if !matches!(name, "find" | "findLast")
            || call.arguments.is_empty()
            || call.arguments.len() > 2
        {
            return;
        }
        let is_function = call
            .arguments
            .first()
            .and_then(Argument::as_expression)
            .is_some_and(|e| {
                matches!(
                    helpers::unwrap_expr(e),
                    Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_)
                )
            });
        if !is_function {
            return;
        }
        if used_as_condition(ctx, node) {
            let Some(property_span) = property_span(member) else {
                return;
            };
            ctx.emit(
                property_span,
                format!(
                    "Use '.some()' instead of '.{name}()' here: only whether an element matches is used, and a falsy match reads as none"
                ),
            );
            return;
        }
        // `const found = list.find(…)` where `found` only ever decides branches.
        let Some((local, first_use)) = condition_only_local(ctx, node) else {
            return;
        };
        let Some(property_span) = property_span(member) else {
            return;
        };
        ctx.emit(
            property_span,
            format!(
                "Use '.some()' instead of '.{name}()': '{local}' is only used as a condition, and a falsy match reads as none"
            ),
        )
        .secondary(first_use, format!("'{local}' is only tested, never used as an element"));
    }
}

/// The local that `node` initializes (`const found = list.find(…)`), when every use of it is a
/// condition: its name and its first use.
fn condition_only_local<'a>(ctx: &Ctx<'a>, node: &AstNode<'a>) -> Option<(&'a str, Span)> {
    let AstKind::VariableDeclarator(declarator) = ctx.outer_parent(node)?.kind() else {
        return None;
    };
    if declarator.kind == VariableDeclarationKind::Var {
        return None;
    }
    let BindingPattern::BindingIdentifier(id) = &declarator.id else {
        return None;
    };
    // A module-level or exported binding may be used elsewhere.
    if !ctx.is_in_function(node) {
        return None;
    }
    let symbol = ctx.binding_symbol(id)?;
    if !ctx.scoping().symbol_redeclarations(symbol).is_empty() {
        return None;
    }
    let mut first_use: Option<Span> = None;
    for reference in ctx.references_of(symbol) {
        if reference.is_write() || reference.is_type() {
            return None;
        }
        let use_node = ctx.node(reference.node_id());
        if !used_as_condition(ctx, use_node) {
            return None;
        }
        let span = use_node.span();
        if first_use.is_none_or(|first| span.start < first.start) {
            first_use = Some(span);
        }
    }
    Some((id.name.as_str(), first_use?))
}

fn property_span(member: &MemberExpression<'_>) -> Option<Span> {
    match member {
        MemberExpression::StaticMemberExpression(m) => Some(m.property.span),
        MemberExpression::ComputedMemberExpression(m) => Some(m.expression.span()),
        MemberExpression::PrivateFieldExpression(_) => None,
    }
}

/// Is the value of `node` only used for its truthiness?
fn used_as_condition(ctx: &Ctx<'_>, node: &AstNode<'_>) -> bool {
    let mut child = node.span();
    for ancestor in ctx.ancestors(node) {
        match ancestor.kind() {
            // Wrappers that pass the value on: `a?.find()`, `x as T`, `x!`.
            AstKind::ChainExpression(_)
            | AstKind::TSAsExpression(_)
            | AstKind::TSSatisfiesExpression(_)
            | AstKind::TSNonNullExpression(_)
            | AstKind::TSTypeAssertion(_)
            | AstKind::ParenthesizedExpression(_) => {}
            AstKind::UnaryExpression(u) => return u.operator == UnaryOperator::LogicalNot,
            AstKind::IfStatement(s) => return s.test.span() == child,
            AstKind::WhileStatement(s) => return s.test.span() == child,
            AstKind::DoWhileStatement(s) => return s.test.span() == child,
            AstKind::ForStatement(s) => return s.test.as_ref().is_some_and(|t| t.span() == child),
            AstKind::ConditionalExpression(c) => return c.test.span() == child,
            AstKind::CallExpression(c) => {
                return c.arguments.len() == 1 && ctx.is_global(&c.callee, "Boolean");
            }
            // `a && b` / `a || b`: the operand is a condition only if the whole expression is one
            // (`x || list.find(…)` as a value returns the element).
            AstKind::LogicalExpression(l) => {
                if l.operator == LogicalOperator::Coalesce {
                    return false;
                }
            }
            _ => return false,
        }
        child = ancestor.span();
    }
    false
}
