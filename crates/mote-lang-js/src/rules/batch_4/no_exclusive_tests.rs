//! `js/no-exclusive-tests`

use crate::prelude::*;

pub struct NoExclusiveTests;

pub static NO_EXCLUSIVE_TESTS: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["tests", "suspicious"],
    remediation_minutes: 1,
    description: r#"Flags focused tests in test files: `.only` on `describe`, `it`, `test`, `context`,
`suite`, or `specify` (including `test.only.each`), and the Jasmine shortcuts `fdescribe` and `fit`.

**Why:** focusing a test is a debugging aid. When it is committed, the runner quietly executes only
the focused tests and skips everything else in the file (or the whole run), so the suite keeps
passing while most of it no longer runs. Failures go unnoticed until someone removes the marker.

**Fix:** remove `.only` (or rename `fit` / `fdescribe` back to `it` / `describe`) before committing.

```js
describe.only('cart', () => {});   // flagged
it.only('adds an item', () => {});  // flagged
fit('removes an item', () => {});   // flagged

describe('cart', () => {});         // compliant
it('adds an item', () => {});       // compliant
```

Only files recognised as tests are checked, and only when the test function is the global one or
is imported (from Jest, Vitest, Mocha, Playwright, …).
"#,
    ..RuleMeta::new(
        "js/no-exclusive-tests",
        "js",
        "Focused tests should not be committed",
    )
};

const TEST_FUNCTIONS: &[&str] = &["describe", "it", "test", "context", "suite", "specify"];

impl JsRule for NoExclusiveTests {
    fn meta(&self) -> &'static RuleMeta {
        &NO_EXCLUSIVE_TESTS
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CallExpression]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.is_test()
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::CallExpression(call) = node.kind() else {
            return;
        };
        match helpers::unwrap_expr(&call.callee) {
            Expression::Identifier(id) if id.name == "fit" || id.name == "fdescribe" => {
                if !is_test_api(ctx, id) {
                    return;
                }
                let plain = if id.name == "fit" { "it" } else { "describe" };
                ctx.emit(
                    id.span,
                    format!(
                        "Replace '{}' with '{plain}': a focused test makes the runner skip every other test",
                        id.name
                    ),
                );
            }
            callee @ Expression::StaticMemberExpression(_) => {
                // Walk the member chain down to its root: `test.only.each` → `test`.
                let mut only: Option<Span> = None;
                let mut current = callee;
                loop {
                    match current {
                        Expression::StaticMemberExpression(m) => {
                            if m.property.name == "only" {
                                only = Some(m.property.span);
                            }
                            current = helpers::unwrap_expr(&m.object);
                        }
                        Expression::Identifier(root) => {
                            let Some(only) = only else {
                                return;
                            };
                            if TEST_FUNCTIONS.contains(&root.name.as_str())
                                && is_test_api(ctx, root)
                            {
                                ctx.emit(
                                    only,
                                    format!(
                                        "Remove '.only' from '{}': it makes the runner skip every other test",
                                        root.name
                                    ),
                                );
                            }
                            return;
                        }
                        _ => return,
                    }
                }
            }
            _ => {}
        }
    }
}

/// The identifier is the test framework's function: a global, or an imported binding.
fn is_test_api(ctx: &Ctx<'_>, id: &IdentifierReference<'_>) -> bool {
    match ctx.reference_symbol(id) {
        None => ctx.is_global_reference(id),
        Some(symbol) => ctx.symbol_flags(symbol).contains(SymbolFlags::Import),
    }
}
