//! `js/unspecific-throw-assertion`

use super::util;
use crate::prelude::*;

pub struct UnspecificThrowAssertion;

pub static UNSPECIFIC_THROW_ASSERTION: RuleMeta = RuleMeta {
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["tests"],
    remediation_minutes: 5,
    description: r#"Flags assertions that code throws without saying what it should throw:
`expect(fn).toThrow()`, `toThrowError()`, `expect(p).rejects.toThrow()`, chai's
`expect(fn).to.throw()` and `should.throw()`, and `assert.throws(fn)` / `assert.rejects(p)` with no
expected error.

**Why:** any error satisfies such an assertion, including the ones the test did not mean to cause:
a `TypeError` from a typo in the test itself, a missing mock, a renamed method. The test keeps
passing while the behavior it was written for is gone.

**Fix:** name the expected error: its class, a message, a pattern, or an object describing it.

```js
expect(() => parse('')).toThrow();                          // flagged
assert.throws(() => parse(''));                             // flagged

expect(() => parse('')).toThrow(SyntaxError);               // compliant
expect(() => parse('')).toThrow(/empty input/);             // compliant
assert.throws(() => parse(''), { name: 'SyntaxError' });    // compliant
```

Not reported: negated assertions (`.not.toThrow()`, `.to.not.throw()`), which need no argument, and
snapshot matchers. Only test files are checked.
"#,
    ..RuleMeta::new(
        "js/unspecific-throw-assertion",
        "js",
        "Assertions that code throws should say what it throws",
    )
};

impl JsRule for UnspecificThrowAssertion {
    fn meta(&self) -> &'static RuleMeta {
        &UNSPECIFIC_THROW_ASSERTION
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CallExpression]
    }

    fn callee_names(&self) -> &'static [&'static str] {
        &[
            "toThrow",
            "toThrowError",
            "throw",
            "throws",
            "Throw",
            "rejects",
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
        let matcher = callee.property.name.as_str();
        if !matches!(
            matcher,
            "toThrow" | "toThrowError" | "throw" | "throws" | "Throw" | "rejects"
        ) {
            return;
        }
        // Node's and chai's `assert.throws(fn)`, `assert.rejects(promise)`.
        if let Expression::Identifier(object) = helpers::unwrap_expr(&callee.object) {
            if object.name == "assert"
                && matches!(matcher, "throws" | "rejects")
                && call.arguments.len() == 1
                && !matches!(call.arguments.first(), Some(Argument::SpreadElement(_)))
                && is_assert_module(ctx, object)
            {
                ctx.emit(
                    callee.property.span,
                    format!(
                        "Pass the expected error to 'assert.{matcher}': any error, even a typo, passes this assertion"
                    ),
                );
            }
            return;
        }
        if !call.arguments.is_empty() || matcher == "rejects" {
            return;
        }
        let (root, members) = match util::callee_chain(&call.callee) {
            Some((root, members)) => (Some(root), members),
            None => (None, member_names(&call.callee)),
        };
        if members.iter().any(|m| matches!(*m, "not" | "Not")) {
            return;
        }
        let is_expect = root.is_some_and(|r| r.name == "expect" && util::is_test_api(ctx, r));
        let applies = match matcher {
            // Jest, Vitest, Jasmine.
            "toThrow" | "toThrowError" => is_expect,
            // chai: `expect(fn).to.throw()`, `fn.should.throw()`.
            _ => (is_expect && members.contains(&"to")) || members.contains(&"should"),
        };
        if applies {
            ctx.emit(
                callee.property.span,
                format!(
                    "Say what '{matcher}' should throw: any error, even a typo, passes this assertion"
                ),
            );
        }
    }
}

/// The member names of a chain whose root is not an identifier (`(() => f()).should.throw`).
fn member_names<'a>(callee: &Expression<'a>) -> Vec<&'a str> {
    let mut names = Vec::new();
    let mut current = helpers::unwrap_expr(callee);
    loop {
        match current {
            Expression::StaticMemberExpression(m) => {
                names.push(m.property.name.as_str());
                current = helpers::unwrap_expr(&m.object);
            }
            Expression::CallExpression(c) => current = helpers::unwrap_expr(&c.callee),
            _ => return names,
        }
    }
}

/// `assert` is the global, an import or `require('assert')`.
fn is_assert_module(ctx: &Ctx<'_>, object: &IdentifierReference<'_>) -> bool {
    match ctx.reference_symbol(object) {
        None => true,
        Some(symbol) => {
            ctx.symbol_flags(symbol).contains(SymbolFlags::Import) || is_require(ctx, symbol)
        }
    }
}

/// `const assert = require('assert')`.
fn is_require(ctx: &Ctx<'_>, symbol: SymbolId) -> bool {
    match ctx.symbol_declaration(symbol).kind() {
        AstKind::VariableDeclarator(d) => matches!(
            d.init.as_ref().map(helpers::unwrap_expr),
            Some(Expression::CallExpression(c)) if c.callee.is_specific_id("require")
        ),
        _ => false,
    }
}
