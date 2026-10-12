//! `js/self-comparing-assertion`

use super::util;
use crate::prelude::*;

pub struct SelfComparingAssertion;

pub static SELF_COMPARING_ASSERTION: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["tests", "suspicious", "copy-paste"],
    remediation_minutes: 5,
    description: r#"Flags test assertions that compare a value with itself: `expect(total).toBe(total)`,
`assert.equal(result, result)`, `expect(a.id).to.equal(a.id)`.

**Why:** such an assertion always passes (or, negated, always fails), whatever the code under test
does. It is nearly always a copy-paste slip — the expected value was meant to be a literal, another
variable, or the result of a different call — and it hides a missing check.

**Fix:** compare against the value the test means to verify.

```js
expect(cart.total).toBe(cart.total);     // flagged: always passes
assert.strictEqual(user.name, user.name); // flagged

expect(cart.total).toBe(30);             // compliant
assert.strictEqual(user.name, 'Ada');    // compliant
```

Only side-effect-free expressions (names, property accesses) are compared; two calls such as
`expect(make()).toEqual(make())` may legitimately return equal values and are not reported. Nor
are assertions in a test whose title says it checks stability (`it('returns the same id on
subsequent calls', …)`): reading a getter twice is then the point of the test.
"#,
    ..RuleMeta::new(
        "js/self-comparing-assertion",
        "js",
        "Assertions should not compare a value with itself",
    )
};

const EXPECT_MATCHERS: &[&str] = &[
    "toBe",
    "toEqual",
    "toStrictEqual",
    "toBeCloseTo",
    "toMatchObject",
    "equal",
    "equals",
    "eql",
    "eq",
    "deepEqual",
    "deepStrictEqual",
    "strictEqual",
    "same",
];

const ASSERT_METHODS: &[&str] = &[
    "equal",
    "equals",
    "strictEqual",
    "deepEqual",
    "deepStrictEqual",
    "notEqual",
    "notStrictEqual",
    "notDeepEqual",
    "notDeepStrictEqual",
    "is",
    "same",
    "eq",
    "sameMembers",
    "sameDeepMembers",
];

/// Callee names the rule acts on: [`EXPECT_MATCHERS`] and [`ASSERT_METHODS`].
const CALLEES: &[&str] = &[
    "toBe",
    "toEqual",
    "toStrictEqual",
    "toBeCloseTo",
    "toMatchObject",
    "equal",
    "equals",
    "eql",
    "eq",
    "deepEqual",
    "deepStrictEqual",
    "strictEqual",
    "same",
    "notEqual",
    "notStrictEqual",
    "notDeepEqual",
    "notDeepStrictEqual",
    "is",
    "sameMembers",
    "sameDeepMembers",
];

impl JsRule for SelfComparingAssertion {
    fn meta(&self) -> &'static RuleMeta {
        &SELF_COMPARING_ASSERTION
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CallExpression]
    }

    fn callee_names(&self) -> &'static [&'static str] {
        CALLEES
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        // Only files that can assert: `assert` or `expect` is a global or declared (imported).
        let named = |n: &str| matches!(n, "assert" | "expect");
        let globals = ctx.scoping().root_unresolved_references();
        globals.contains_key("assert")
            || globals.contains_key("expect")
            || ctx.scoping().symbol_names().any(named)
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::CallExpression(call) = node.kind() else {
            return;
        };
        let Expression::StaticMemberExpression(member) = &call.callee else {
            return;
        };
        let method = member.property.name.as_str();
        let on_assert = matches!(&member.object, Expression::Identifier(id) if id.name == "assert");
        let pair = if on_assert && ASSERT_METHODS.contains(&method) {
            // assert.equal(actual, expected)
            match (call.arguments.first(), call.arguments.get(1)) {
                (Some(a), Some(b)) => (helpers::argument_expr(a), helpers::argument_expr(b)),
                _ => return,
            }
        } else if EXPECT_MATCHERS.contains(&method) {
            // expect(actual)[.not|.to|.be|.deep…].toBe(expected)
            let Some(actual) = expect_subject(&member.object) else {
                return;
            };
            (
                Some(actual),
                call.arguments.first().and_then(helpers::argument_expr),
            )
        } else {
            return;
        };
        let (Some(actual), Some(expected)) = pair else {
            return;
        };
        if !util::same_pure_expr(actual, expected)
            || !util::mentions_variable(actual)
            || tests_stability(ctx, node)
        {
            return;
        }
        let operand = util::excerpt(ctx.text(actual.span()));
        ctx.emit(
            call.span,
            format!("This assertion compares '{operand}' with itself, so its outcome is fixed; compare with the expected value"),
        );
    }
}

/// The argument of the `expect(…)` call at the root of a matcher chain.
fn expect_subject<'b, 'a>(expr: &'b Expression<'a>) -> Option<&'b Expression<'a>> {
    match expr {
        Expression::CallExpression(call) => {
            let Expression::Identifier(id) = &call.callee else {
                return None;
            };
            if id.name != "expect" {
                return None;
            }
            call.arguments.first().and_then(helpers::argument_expr)
        }
        Expression::StaticMemberExpression(m) => {
            if matches!(
                m.property.name.as_str(),
                "not" | "to" | "be" | "deep" | "strict" | "been" | "is" | "that" | "and" | "own"
            ) {
                expect_subject(&m.object)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Is the assertion inside a test whose title says it checks that a value is stable across reads
/// (`it('returns the same id on subsequent calls', …)`)? Reading a getter twice is then the point.
fn tests_stability(ctx: &Ctx<'_>, node: &AstNode<'_>) -> bool {
    const WORDS: &[&str] = &[
        "same",
        "stable",
        "consistent",
        "subsequent",
        "identical",
        "cache",
        "idempot",
        "memo",
        "again",
        "twice",
        "repeat",
    ];
    for ancestor in ctx.ancestors(node) {
        let AstKind::CallExpression(call) = ancestor.kind() else {
            continue;
        };
        let is_test = match &call.callee {
            Expression::Identifier(id) => matches!(id.name.as_str(), "it" | "test" | "specify"),
            Expression::StaticMemberExpression(m) => matches!(
                &m.object,
                Expression::Identifier(id) if matches!(id.name.as_str(), "it" | "test")
            ),
            _ => false,
        };
        if !is_test {
            continue;
        }
        let Some(title) = call.arguments.first().and_then(helpers::argument_expr) else {
            return false;
        };
        let title = ctx.text(title.span()).to_ascii_lowercase();
        return WORDS.iter().any(|w| title.contains(w));
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callee_names_cover_the_rule() {
        for name in EXPECT_MATCHERS.iter().chain(ASSERT_METHODS) {
            assert!(CALLEES.contains(name), "{name} missing from CALLEES");
        }
    }
}
