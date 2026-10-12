//! `js/test-file-without-tests`

use super::util;
use crate::prelude::*;

pub struct TestFileWithoutTests;

pub static TEST_FILE_WITHOUT_TESTS: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Incubating,
    tags: &["tests"],
    remediation_minutes: 10,
    description: r#"Flags a file named as a test (`*.test.*`, `*.spec.*`) that declares suites (`describe`) but
registers no test case: no `it`, `test` or `specify` call anywhere in it.

**Why:** the runner picks the file up, runs nothing and reports success, so the file looks like
coverage for the code next to it while checking nothing. This usually happens when every test of
the file was deleted or commented out, or when a suite only contains `describe` blocks whose
cases were never written.

**Fix:** write the missing tests, restore the ones that were commented out, or delete the file (and
move any shared helpers into a file that is not named as a test).

```js
// cart.spec.js — flagged: the only test was commented out
describe('cart', () => {
  // it('adds an item', () => { … });
});
```

Only the file name decides: helper files in test folders are not tests. Not reported: files
without any `describe` (scripts, data and helper files that carry a test name), files whose
statements are only imports and exports (suites assembled from other files), and files that call
an imported function at the top level or inside a suite (custom test factories such as
`runConformanceTests(…)` or `testRule(…)` may register the cases).
"#,
    ..RuleMeta::new(
        "js/test-file-without-tests",
        "js",
        "Test files should contain tests",
    )
};

impl JsRule for TestFileWithoutTests {
    fn meta(&self) -> &'static RuleMeta {
        &TEST_FILE_WITHOUT_TESTS
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        util::has_test_file_name(ctx.path())
    }

    fn run_once<'a>(&self, ctx: &mut Ctx<'a>) {
        let Some(program) = util::program(ctx) else {
            return;
        };
        let Some(first) = program.body.first() else {
            return;
        };
        // Only imports and exports: a suite assembled from other files.
        if program.body.iter().all(|s| {
            matches!(
                s,
                Statement::ImportDeclaration(_)
                    | Statement::ExportAllDeclaration(_)
                    | Statement::ExportNamedDeclaration(_)
                    | Statement::ExportDefaultDeclaration(_)
                    | Statement::TSImportEqualsDeclaration(_)
            )
        }) {
            return;
        }
        let mut has_suite = false;
        for node in ctx.nodes().iter() {
            let AstKind::CallExpression(call) = node.kind() else {
                continue;
            };
            let Some(root) = util::callee_root(&call.callee) else {
                continue;
            };
            let name = root.name.as_str();
            if matches!(name, "describe" | "suite" | "context") && util::is_test_api(ctx, root) {
                has_suite = true;
            }
            if matches!(
                name,
                "it" | "test"
                    | "specify"
                    | "xit"
                    | "fit"
                    | "xtest"
                    | "xspecify"
                    | "bench"
                    | "Scenario"
                    | "Given"
                    | "When"
                    | "Then"
            ) {
                return;
            }
            // A call to an imported function, at the top level or directly in a suite, may
            // register cases (test factories).
            if !is_suite_level_statement(ctx, node) {
                continue;
            }
            if let Some(symbol) = ctx.reference_symbol(root) {
                if ctx.symbol_flags(symbol).contains(SymbolFlags::Import) && !is_harness(name) {
                    return;
                }
            } else if !is_known_global(name) {
                // An unknown global (a helper installed by the runner's setup file).
                return;
            }
        }
        // Without a suite the file is a script, a data file or a helper that happens to carry a
        // test name; an empty suite is the clear case.
        if !has_suite {
            return;
        }
        let span = Span::new(first.span().start, first.span().start);
        ctx.emit(
            span,
            "Add test cases to this test file, or rename it: it registers no tests",
        );
    }
}

/// Is the call a statement of the program or of a suite callback (`describe('…', () => { f(); })`)?
fn is_suite_level_statement(ctx: &Ctx<'_>, call: &AstNode<'_>) -> bool {
    let mut current = call;
    let stmt = loop {
        let Some(parent) = ctx.parent(current) else {
            return false;
        };
        match parent.kind() {
            AstKind::AwaitExpression(_) => current = parent,
            AstKind::ExpressionStatement(_) => break parent,
            _ => return false,
        }
    };
    let Some(container) = ctx.parent(stmt) else {
        return false;
    };
    match container.kind() {
        AstKind::Program(_) => true,
        AstKind::FunctionBody(_) => {
            let Some(function) = ctx.parent(container) else {
                return false;
            };
            let Some(AstKind::CallExpression(suite)) =
                ctx.outer_parent(function).map(AstNode::kind)
            else {
                return false;
            };
            util::callee_root(&suite.callee)
                .is_some_and(|root| matches!(root.name.as_str(), "describe" | "suite" | "context"))
        }
        _ => false,
    }
}

/// Imported names that never register test cases.
fn is_harness(name: &str) -> bool {
    matches!(
        name,
        "describe"
            | "suite"
            | "context"
            | "beforeEach"
            | "afterEach"
            | "beforeAll"
            | "afterAll"
            | "before"
            | "after"
            | "expect"
            | "vi"
            | "jest"
    )
}

/// Globals that never register test cases.
fn is_known_global(name: &str) -> bool {
    matches!(
        name,
        "describe"
            | "suite"
            | "context"
            | "beforeEach"
            | "afterEach"
            | "beforeAll"
            | "afterAll"
            | "before"
            | "after"
            | "expect"
            | "jest"
            | "vi"
            | "require"
            | "console"
            | "Object"
            | "Array"
            | "JSON"
            | "Promise"
            | "Math"
            | "String"
            | "Number"
            | "Symbol"
            | "Date"
            | "setTimeout"
            | "process"
            | "xdescribe"
            | "fdescribe"
    )
}
