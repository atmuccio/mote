//! `js/valid-expect`

use super::util;
use crate::prelude::*;

pub struct ValidExpect;

pub static VALID_EXPECT: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["tests", "suspicious"],
    remediation_minutes: 2,
    description: r#"Flags test expectations that never assert anything: `expect(value);` on its own, or a matcher
that is named but not called, such as `expect(value).toBeTruthy;`.

**Why:** `expect(…)` only builds an expectation object; nothing is checked until a matcher is
*called*. Without the call the test passes no matter what the value is, so it gives false
confidence — a broken feature ships with a green test.

**Fix:** call the intended matcher: `expect(value).toBeTruthy()`.

```js
expect(result);                       // flagged: no matcher
expect(list.length).toBeGreaterThan;  // flagged: matcher not called
expect(user).not;                     // flagged

expect(result).toBeDefined();         // compliant
expect(list.length).toBeGreaterThan(0); // compliant
expect(flag).to.be.true;              // compliant: Chai property assertion
```

`expect(…)` alone is only reported when its subject has no side effects (names, property reads,
comparisons): `expect(assertValid(x));` or `expect(obj.should.eql(y));` asserts through the call.
Chai-style chains that end in a lowercase property (`.to.be.true`, `.to.exist`) assert on access
and are not reported; only Jest/Vitest-style matchers (`toBe…`, `toEqual`, `toHave…`, …) and the
`not`/`resolves`/`rejects` modifiers must be followed by a call. Chai `expect` and `should` chains
that stop at a word which asserts nothing on its own — a connecting word (`.to.be`, `.have`,
`.that`), a flag (`.not`, `.deep`), or an assertion method that is not called (`.equal`,
`.include`, `.a`) — are reported too:

```js
expect(result).to.be;                 // flagged: no assertion
user.should.have.property;            // flagged: property() not called
```

A locally defined `expect`
function is not checked, nor is an arrow function that returns the expectation for its caller to
complete (`const expectName = () => expect(user.name);`).
"#,
    ..RuleMeta::new(
        "js/valid-expect",
        "js",
        "Expectations should call a matcher",
    )
};

impl JsRule for ValidExpect {
    fn meta(&self) -> &'static RuleMeta {
        &VALID_EXPECT
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.scoping()
            .root_unresolved_references()
            .contains_key("expect")
            || ctx.scoping().symbol_names().any(|n| n == "expect")
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::ExpressionStatement]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::ExpressionStatement(stmt) = node.kind() else {
            return;
        };
        // An arrow function's expression body is also an expression statement: `() => expect(x)`
        // returns the expectation for the caller to complete.
        if let Some(parent) = ctx.parent(node) {
            if matches!(parent.kind(), AstKind::FunctionBody(_))
                && matches!(ctx.parent_kind(parent), Some(AstKind::ArrowFunctionExpression(a)) if a.expression)
            {
                return;
            }
        }
        let expr = helpers::unwrap_expr(&stmt.expression);
        match expr {
            Expression::CallExpression(call) => {
                // `expect(check())` may do its work through the call (an assertion helper, a
                // `.should` chain, a "does not throw" check): only side-effect-free subjects.
                let pure_subject = call
                    .arguments
                    .first()
                    .and_then(helpers::argument_expr)
                    .is_some_and(util::is_pure);
                if pure_subject && is_expect_call(ctx, call) {
                    ctx.emit(
                        call.span,
                        "Call a matcher on this expectation: 'expect(…)' alone asserts nothing",
                    );
                }
            }
            Expression::StaticMemberExpression(member) => {
                let name = member.property.name.as_str();
                if CHAI_NON_ASSERTING.contains(&name) && is_chai_chain(ctx, &member.object) {
                    ctx.emit(
                        member.property.span,
                        format!(
                            "Complete this assertion: the chain ends at '.{name}', which asserts nothing on its own"
                        ),
                    );
                    return;
                }
                let modifier = matches!(name, "not" | "resolves" | "rejects");
                if !modifier && !is_matcher_name(name) {
                    return;
                }
                if !chain_starts_with_expect(ctx, &member.object) {
                    return;
                }
                let message = if modifier {
                    format!(
                        "Complete this expectation: '.{name}' must be followed by a matcher call"
                    )
                } else {
                    format!("Call the matcher '{name}()': as written the assertion never runs")
                };
                ctx.emit(member.property.span, message);
            }
            _ => {}
        }
    }
}

/// `expect(x)` where `expect` is the test framework's global (or an import), not a local function.
fn is_expect_call(ctx: &Ctx<'_>, call: &CallExpression<'_>) -> bool {
    let Expression::Identifier(id) = &call.callee else {
        return false;
    };
    if id.name != "expect" || call.arguments.is_empty() || call.arguments.len() > 2 {
        return false;
    }
    match ctx.reference_symbol(id) {
        None => true,
        Some(symbol) => ctx.symbol_flags(symbol).contains(SymbolFlags::Import),
    }
}

/// Chai words that do not assert when the chain stops at them: connecting words, flags, and
/// assertion methods (which assert only when called).
const CHAI_NON_ASSERTING: &[&str] = &[
    // connecting words
    "to",
    "be",
    "been",
    "is",
    "that",
    "which",
    "and",
    "has",
    "have",
    "with",
    "at",
    "of",
    "same",
    "but",
    "does",
    "still",
    "also",
    // flags
    "deep",
    "nested",
    "own",
    "ordered",
    "any",
    "all",
    "itself",
    // methods
    "a",
    "an",
    "include",
    "includes",
    "contain",
    "contains",
    "equal",
    "equals",
    "eq",
    "eql",
    "eqls",
    "above",
    "gt",
    "greaterThan",
    "least",
    "gte",
    "greaterThanOrEqual",
    "below",
    "lt",
    "lessThan",
    "most",
    "lte",
    "lessThanOrEqual",
    "within",
    "instanceof",
    "instanceOf",
    "property",
    "ownProperty",
    "haveOwnProperty",
    "ownPropertyDescriptor",
    "length",
    "lengthOf",
    "match",
    "matches",
    "string",
    "keys",
    "key",
    "throw",
    "throws",
    "Throw",
    "respondTo",
    "respondsTo",
    "satisfy",
    "satisfies",
    "closeTo",
    "approximately",
    "members",
    "oneOf",
    "change",
    "changes",
    "increase",
    "increases",
    "decrease",
    "decreases",
    "by",
];

/// Is the expression a Chai chain: it starts with `expect(…)` (the test framework's) or goes
/// through a `.should` property, and continues with `.word` and `.method(…)` links?
fn is_chai_chain(ctx: &Ctx<'_>, expr: &Expression<'_>) -> bool {
    let mut current = expr;
    loop {
        current = match current {
            Expression::StaticMemberExpression(m) => {
                if m.property.name == "should" {
                    return true;
                }
                &m.object
            }
            Expression::CallExpression(call) => {
                if is_expect_call(ctx, call) {
                    return true;
                }
                &call.callee
            }
            _ => return false,
        };
    }
}

/// Jest/Vitest matcher names: `toBe`, `toEqual`, `toHaveBeenCalledWith`, …
fn is_matcher_name(name: &str) -> bool {
    name.strip_prefix("to")
        .and_then(|rest| rest.chars().next())
        .is_some_and(char::is_uppercase)
}

/// Is the expression `expect(…)`, possibly followed by `.not` / `.resolves` / `.rejects`?
fn chain_starts_with_expect(ctx: &Ctx<'_>, expr: &Expression<'_>) -> bool {
    match expr {
        Expression::CallExpression(call) => is_expect_call(ctx, call),
        Expression::StaticMemberExpression(m) => {
            matches!(m.property.name.as_str(), "not" | "resolves" | "rejects")
                && chain_starts_with_expect(ctx, &m.object)
        }
        _ => false,
    }
}
