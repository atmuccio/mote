//! `js/no-empty-pattern`

use crate::prelude::*;

pub struct NoEmptyPattern;

pub static NO_EMPTY_PATTERN: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["suspicious"],
    remediation_minutes: 2,
    description: r#"Flags destructuring patterns that bind nothing: `const {} = obj`, `const [] = list`,
`function f({}) {}`, or a nested `{ a: {} }`.

**Why:** an empty pattern declares no variables, so it has no effect beyond checking that the
value is not `null`/`undefined`. It usually comes from a typo — `{ a: {} }` where a default value
`{ a = {} }` was meant — or from a refactoring that removed the last binding.

**Fix:** name the values you need, use a default value (`{ a = {} }`), or remove the pattern.

```js
const { settings: {} } = config;   // flagged: binds nothing
const { settings = {} } = config;  // compliant: default value
function render({ title }) {}      // compliant
```

An empty object pattern as the first parameter of a function that takes more parameters, or of a
callback passed to `test(…)`, `it(…)` or their hooks (`test.beforeAll(async ({}, testInfo) => …)`),
is not reported: test runners such as Playwright read the fixtures a test needs from that first
parameter and require it to be a destructuring pattern even when it names none.
"#,
    ..RuleMeta::new(
        "js/no-empty-pattern",
        "js",
        "Destructuring patterns should not be empty",
    )
};

impl JsRule for NoEmptyPattern {
    fn meta(&self) -> &'static RuleMeta {
        &NO_EMPTY_PATTERN
    }

    fn types(&self) -> &'static [AstType] {
        &[
            AstType::ObjectPattern,
            AstType::ArrayPattern,
            AstType::ObjectAssignmentTarget,
            AstType::ArrayAssignmentTarget,
        ]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let (span, what) = match node.kind() {
            AstKind::ObjectPattern(p) if p.properties.is_empty() && p.rest.is_none() => {
                (p.span, "object")
            }
            AstKind::ArrayPattern(p) if p.elements.is_empty() && p.rest.is_none() => {
                (p.span, "array")
            }
            AstKind::ObjectAssignmentTarget(p) if p.properties.is_empty() && p.rest.is_none() => {
                (p.span, "object")
            }
            AstKind::ArrayAssignmentTarget(p) if p.elements.is_empty() && p.rest.is_none() => {
                (p.span, "array")
            }
            _ => return,
        };
        if what == "object" && is_fixture_placeholder(ctx, node) {
            return;
        }
        ctx.emit(
            span,
            format!("Remove this empty {what} pattern or name the values to extract"),
        );
    }
}

/// `async ({}, testInfo) => …`: test runners that inject fixtures by reading the names in the
/// first parameter (Playwright) require it to be a destructuring pattern, even an empty one. An
/// empty object pattern as the first parameter of a function with more parameters, or of a
/// callback passed to `test(…)` / `test.beforeEach(…)`-style calls, is that placeholder.
fn is_fixture_placeholder(ctx: &Ctx<'_>, pattern: &AstNode<'_>) -> bool {
    let Some(param) = ctx.parent(pattern) else {
        return false;
    };
    if !matches!(param.kind(), AstKind::FormalParameter(_)) {
        return false;
    }
    let Some(params_node) = ctx.parent(param) else {
        return false;
    };
    let AstKind::FormalParameters(params) = params_node.kind() else {
        return false;
    };
    if params.items.first().map(GetSpan::span) != Some(param.span()) {
        return false;
    }
    if params.items.len() > 1 || params.rest.is_some() {
        return true;
    }
    // `test('…', async ({}) => …)`, `test.beforeAll(async ({}) => …)`
    let Some(function) = ctx.parent(params_node) else {
        return false;
    };
    let Some(call) = ctx.outer_parent(function) else {
        return false;
    };
    let AstKind::CallExpression(call) = call.kind() else {
        return false;
    };
    let root = match helpers::unwrap_expr(&call.callee) {
        Expression::Identifier(id) => Some(id.name.as_str()),
        Expression::StaticMemberExpression(m) => match helpers::unwrap_expr(&m.object) {
            Expression::Identifier(id) => Some(id.name.as_str()),
            _ => None,
        },
        _ => None,
    };
    matches!(root, Some("test" | "it" | "setup" | "teardown"))
}
