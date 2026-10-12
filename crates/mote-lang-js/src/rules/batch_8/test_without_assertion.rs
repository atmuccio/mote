//! `js/test-without-assertion`

use super::util;
use crate::prelude::*;

pub struct TestWithoutAssertion;

pub static TEST_WITHOUT_ASSERTION: RuleMeta = RuleMeta {
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["tests"],
    remediation_minutes: 10,
    description: r#"Flags test cases (`it`, `test`, `specify`) whose body contains no assertion: no `expect`,
`assert`, `should`, verification or failure call, directly or through a helper function of the
same file.

**Why:** a test without an assertion passes as long as nothing throws. It adds to the test count
and to coverage while checking almost nothing, so a regression in what the code computes goes
unnoticed and the suite gives false confidence.

**Fix:** assert on the outcome (`expect(result).toEqual(…)`, `assert.strictEqual(…)`), verify the
expected interactions (`expect(spy).toHaveBeenCalledWith(…)`), or, when the test really only
checks that nothing throws, say so (`expect(() => run()).not.toThrow()`).

```ts
it('loads the orders', () => {            // flagged
  component.ngOnInit();                   // component = fixture.componentInstance
  fixture.detectChanges();
});

it('loads the orders', () => {            // compliant
  component.ngOnInit();
  fixture.detectChanges();
  expect(component.orders.length).toBe(2);
});
```

A call counts as an assertion when a name in it starts with `expect`, `assert`, `should`, `verify`,
`fail` or `check` (`expect(…)`, `assert.equal`, `x.should.equal`, `sinon.assert…`,
`httpMock.expectOne`, `request(app).expect(200)`), when it is a Testing Library query that throws
when nothing matches (`getBy…`, `findBy…`), or a Cypress command; a `throw` statement counts too.
Functions and classes of the same file are followed, and so are the values of local variables.
Code the rule cannot see is assumed to assert: functions, classes and objects imported from other
modules (except the rendering and harness helpers of Testing Library, Angular, React, Enzyme, Vue,
Jest, Vitest and Sinon), unknown globals set up by the test runner, and methods reached through
`this` (Mocha contexts, page objects).

Only test files are checked. Not reported: test callbacks that take parameters (`done`, a test
context `t`, Playwright fixtures), empty tests (placeholders, or tests disabled by commenting out
their body), compile-time type checks (a `@ts-expect-error` comment, a type imported with
`import type`, a call to a local function with an empty body), skipped and todo tests, tests whose
callback is not written inline, and tests that call `pending()` or `skip()`.
"#,
    ..RuleMeta::new(
        "js/test-without-assertion",
        "js",
        "Tests should contain an assertion",
    )
};

/// Name prefixes of assertion calls.
const ASSERT_PREFIXES: &[&str] = &["expect", "assert", "should", "verify", "fail", "check"];

/// Globals that never assert.
const PLAIN_GLOBALS: &[&str] = &[
    "console",
    "JSON",
    "Object",
    "Array",
    "Promise",
    "Math",
    "Date",
    "String",
    "Number",
    "Boolean",
    "Symbol",
    "Map",
    "Set",
    "WeakMap",
    "RegExp",
    "Error",
    "BigInt",
    "Reflect",
    "Buffer",
    "URL",
    "require",
    "setTimeout",
    "setInterval",
    "setImmediate",
    "clearTimeout",
    "clearInterval",
    "queueMicrotask",
    "structuredClone",
    "parseInt",
    "parseFloat",
    "encodeURIComponent",
    "decodeURIComponent",
    "document",
    "window",
    "globalThis",
    "process",
    "jest",
    "vi",
    "jasmine",
    "spyOn",
    "sinon",
    "describe",
    "it",
    "test",
    "beforeEach",
    "afterEach",
    "beforeAll",
    "afterAll",
    "before",
    "after",
    "fakeAsync",
    "waitForAsync",
    "tick",
    "flush",
    "flushMicrotasks",
    "inject",
    "TestBed",
];

/// Packages whose functions never assert (test harnesses, rendering and event helpers).
const PLAIN_PACKAGES: &[&str] = &[
    "@angular/core/testing",
    "@angular/core",
    "@angular/platform-browser",
    "@angular/platform-browser/animations",
    "@angular/common/http/testing",
    "@angular/router/testing",
    "@testing-library/react",
    "@testing-library/angular",
    "@testing-library/vue",
    "@testing-library/user-event",
    "@testing-library/react-hooks",
    "react",
    "react-dom",
    "react-dom/client",
    "react-dom/test-utils",
    "enzyme",
    "@vue/test-utils",
    "vitest",
    "@jest/globals",
    "jest-mock",
    "sinon",
    "rxjs",
    "rxjs/operators",
];

impl JsRule for TestWithoutAssertion {
    fn meta(&self) -> &'static RuleMeta {
        &TEST_WITHOUT_ASSERTION
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CallExpression]
    }

    fn callee_names(&self) -> &'static [&'static str] {
        &["it", "test", "specify", "only", "concurrent", "serial"]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.is_test()
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::CallExpression(call) = node.kind() else {
            return;
        };
        // The root name first: collecting the chain's member names allocates.
        if !util::callee_root(&call.callee)
            .is_some_and(|root| matches!(root.name.as_str(), "it" | "test" | "specify"))
        {
            return;
        }
        let Some((root, members)) = util::callee_chain(&call.callee) else {
            return;
        };
        if members.iter().any(|m| {
            matches!(
                *m,
                "skip" | "todo" | "each" | "skipIf" | "runIf" | "failing" | "fixme" | "fail"
            )
        }) || !members
            .iter()
            .all(|m| matches!(*m, "only" | "concurrent" | "serial"))
        {
            return;
        }
        // `test.each(table)(name, fn)` has a call in the callee: not a plain test.
        if !matches!(
            helpers::unwrap_expr(&call.callee),
            Expression::Identifier(_) | Expression::StaticMemberExpression(_)
        ) {
            return;
        }
        if !util::is_test_api(ctx, root) || call.arguments.len() < 2 {
            return;
        }
        let Some(callback) = call.arguments.iter().rev().find_map(|a| {
            let e = helpers::unwrap_expr(a.as_expression()?);
            match e {
                Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_) => {
                    Some(e)
                }
                _ => None,
            }
        }) else {
            return;
        };
        let (has_params, empty) = match callback {
            Expression::ArrowFunctionExpression(a) => (
                !a.params.items.is_empty() || a.params.rest.is_some(),
                a.body.statements.is_empty(),
            ),
            Expression::FunctionExpression(f) => (
                !f.params.items.is_empty() || f.params.rest.is_some(),
                f.body.as_ref().is_none_or(|b| b.statements.is_empty()),
            ),
            _ => return,
        };
        // An empty body is a placeholder or a test disabled by commenting it out.
        if has_params || empty {
            return;
        }
        // `// @ts-expect-error` makes the compiler the assertion.
        let span = callback.span();
        // Comments are sorted by position: only those starting inside the callback can be in it.
        let comments = ctx.comments();
        let first = comments.partition_point(|c| c.span.start < span.start);
        if comments[first..]
            .iter()
            .take_while(|c| c.span.start <= span.end)
            .any(|c| c.span.end <= span.end && ctx.text(c.span).contains("@ts-expect-error"))
        {
            return;
        }
        let Some(callback_node) =
            util::descendants(ctx, node).find(|n| n.span() == callback.span())
        else {
            return;
        };
        let mut visited: Vec<SymbolId> = Vec::new();
        if may_assert(ctx, callback_node, &mut visited, 0) {
            return;
        }
        ctx.emit(
            root.span,
            "Add an assertion to this test: it passes as long as nothing throws",
        );
    }
}

/// Does the code under `root` assert, or call something that may?
fn may_assert(ctx: &Ctx<'_>, root: &AstNode<'_>, visited: &mut Vec<SymbolId>, depth: u32) -> bool {
    if depth > 4 {
        return true;
    }
    for node in util::descendants(ctx, root) {
        let callee = match node.kind() {
            AstKind::ThrowStatement(_) => return true,
            AstKind::StaticMemberExpression(m) => {
                if is_assert_name(m.property.name.as_str()) {
                    return true;
                }
                continue;
            }
            AstKind::IdentifierReference(id) => {
                let name = id.name.as_str();
                if is_assert_name(name) || name == "cy" {
                    return true;
                }
                continue;
            }
            // A type imported with `import type`: the test checks types at compile time.
            AstKind::TSTypeReference(t) => {
                if let TSTypeName::IdentifierReference(id) = &t.type_name {
                    if is_type_only_import(ctx, id) {
                        return true;
                    }
                }
                continue;
            }
            AstKind::CallExpression(call) => &call.callee,
            AstKind::NewExpression(new) => &new.callee,
            _ => continue,
        };
        if callee_may_assert(ctx, callee, visited, depth) {
            return true;
        }
    }
    false
}

fn is_assert_name(name: &str) -> bool {
    ASSERT_PREFIXES.iter().any(|p| {
        name.strip_prefix(p).is_some_and(|rest| {
            rest.is_empty() || !rest.starts_with(|c: char| c.is_ascii_lowercase())
        })
    }) || ["getBy", "getAllBy", "findBy", "findAllBy"]
        .iter()
        .any(|p| name.starts_with(p))
        || matches!(name, "skip" | "pending")
}

/// May the called code assert? Code the rule cannot see is assumed to; code of the same file is
/// followed.
fn callee_may_assert(
    ctx: &Ctx<'_>,
    callee: &Expression<'_>,
    visited: &mut Vec<SymbolId>,
    depth: u32,
) -> bool {
    // The root of the callee chain: `helper()`, `helpers.check()`, `new Page().open()`.
    let mut current = helpers::unwrap_expr(callee);
    loop {
        current = match current {
            Expression::StaticMemberExpression(m) => helpers::unwrap_expr(&m.object),
            Expression::ComputedMemberExpression(m) => helpers::unwrap_expr(&m.object),
            Expression::PrivateFieldExpression(m) => helpers::unwrap_expr(&m.object),
            Expression::CallExpression(c) => helpers::unwrap_expr(&c.callee),
            Expression::NewExpression(n) => helpers::unwrap_expr(&n.callee),
            Expression::ChainExpression(_) => return true,
            Expression::AwaitExpression(a) => helpers::unwrap_expr(&a.argument),
            _ => break,
        };
    }
    match current {
        Expression::Identifier(id) => match ctx.reference_symbol(id) {
            None => !PLAIN_GLOBALS.contains(&id.name.as_str()),
            Some(symbol) => symbol_may_assert(ctx, symbol, visited, depth),
        },
        // Values built on the spot: their methods are built-ins.
        Expression::StringLiteral(_)
        | Expression::TemplateLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::ArrayExpression(_)
        | Expression::ObjectExpression(_)
        | Expression::RegExpLiteral(_) => false,
        // `this` (a Mocha context, a page object), `super`, and anything else: unknown.
        _ => true,
    }
}

/// May code reached through this binding assert?
fn symbol_may_assert(
    ctx: &Ctx<'_>,
    symbol: SymbolId,
    visited: &mut Vec<SymbolId>,
    depth: u32,
) -> bool {
    if visited.contains(&symbol) {
        return false;
    }
    visited.push(symbol);
    let declaration = ctx.symbol_declaration(symbol);
    match declaration.kind() {
        AstKind::ImportSpecifier(_)
        | AstKind::ImportDefaultSpecifier(_)
        | AstKind::ImportNamespaceSpecifier(_) => {
            import_source(ctx, declaration).is_none_or(source_may_assert)
        }
        // Functions and classes of the file: look inside.
        // A function with an empty body called with typed arguments is a compile-time check.
        AstKind::Function(f) if f.body.as_ref().is_some_and(|b| b.statements.is_empty()) => true,
        AstKind::Function(_) | AstKind::Class(_) => {
            may_assert(ctx, declaration, visited, depth + 1)
        }
        AstKind::VariableDeclarator(d) => {
            // `const helpers = require('./helpers')`
            if let Some(Expression::CallExpression(c)) = d.init.as_ref().map(helpers::unwrap_expr) {
                if c.callee.is_specific_id("require") {
                    return c
                        .arguments
                        .first()
                        .and_then(Argument::as_expression)
                        .and_then(helpers::string_value)
                        .is_none_or(source_may_assert);
                }
            }
            // The initializer and every later assignment decide what the variable holds.
            let mut known = d.init.is_some();
            if let Some(init) = &d.init {
                if callee_may_assert(ctx, init, visited, depth + 1) {
                    return true;
                }
            }
            if may_assert(ctx, declaration, visited, depth + 1) {
                return true;
            }
            for reference in ctx.references_of(symbol) {
                if !reference.is_write() {
                    continue;
                }
                let Some(assignment) = ctx
                    .ancestors(ctx.node(reference.node_id()))
                    .find(|a| matches!(a.kind(), AstKind::AssignmentExpression(_)))
                else {
                    return true;
                };
                known = true;
                if let AstKind::AssignmentExpression(a) = assignment.kind() {
                    if callee_may_assert(ctx, &a.right, visited, depth + 1) {
                        return true;
                    }
                }
                if may_assert(ctx, assignment, visited, depth + 1) {
                    return true;
                }
            }
            !known
        }
        // Parameters, catch bindings, enums, …: unknown.
        _ => true,
    }
}

/// Does the name come from an `import type` declaration or specifier?
fn is_type_only_import(ctx: &Ctx<'_>, id: &IdentifierReference<'_>) -> bool {
    let Some(symbol) = ctx.reference_symbol(id) else {
        return false;
    };
    let declaration = ctx.symbol_declaration(symbol);
    match declaration.kind() {
        AstKind::ImportSpecifier(s) if s.import_kind.is_type() => true,
        AstKind::ImportSpecifier(_)
        | AstKind::ImportDefaultSpecifier(_)
        | AstKind::ImportNamespaceSpecifier(_) => ctx
            .ancestors(declaration)
            .any(|a| matches!(a.kind(), AstKind::ImportDeclaration(d) if d.import_kind.is_type())),
        _ => false,
    }
}

/// The module an import binding comes from.
fn import_source<'a>(ctx: &Ctx<'a>, declaration: &AstNode<'a>) -> Option<&'a str> {
    ctx.ancestors(declaration).find_map(|a| match a.kind() {
        AstKind::ImportDeclaration(d) => Some(d.source.value.as_str()),
        _ => None,
    })
}

/// Any module may export an assertion helper, except the known harness packages.
fn source_may_assert(source: &str) -> bool {
    !PLAIN_PACKAGES.contains(&source)
}
