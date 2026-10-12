//! `js/max-nested-functions`

use super::util;
use crate::prelude::*;

pub struct MaxNestedFunctions;

pub static MAX_NESTED_FUNCTIONS: RuleMeta = RuleMeta {
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["complexity", "readability"],
    remediation_minutes: 20,
    params: &[ParamSpec {
        name: "max",
        default: ParamDefault::Int(4),
        description: "Maximum depth of nested functions (the function itself counts as a level).",
    }],
    description: r#"Flags a function nested more than `max` (default 4) levels deep, counting the function itself
(with the default, a fifth function inside four enclosing ones): callback pyramids, closures
inside closures inside methods.

**Why:** every level of nesting adds a scope whose variables stay visible below it and an
indentation level the reader has to keep track of. Deep nests make it hard to see which variables
a callback captures, in which order things run, and where errors go. They usually grow one small
callback at a time, which is why a limit helps.

**Fix:** give inner callbacks names and move them out to the module or class level, pass what they
need as parameters, or flatten promise chains with `async`/`await`.

```js
load(id, (err, doc) => {                 // 1
  parse(doc, (err, tree) => {            // 2
    render(tree, (err, html) => {        // 3
      save(html, (err) => {              // 4
        notify(() => done());            // 5: flagged
      });
    });
  });
});

const doc = await load(id);              // compliant
const html = await render(await parse(doc));
await save(html);
await notify();
```

Only the outermost function past the limit is reported, once per nest. Not counted: arrow
functions whose body is a single expression (`x => x.id`), which open no block, functions called
on the spot (module wrappers such as `(function () { … })()`), functions handed to such a
wrapper (UMD module factories: `(function (factory) { … }(function ($) { … }))`), AMD
`define(…)` factories, and in test files the callbacks of `describe`, `it`, `test` and their
hooks.
"#,
    ..RuleMeta::new(
        "js/max-nested-functions",
        "js",
        "Functions should not be nested too deeply",
    )
};

impl JsRule for MaxNestedFunctions {
    fn meta(&self) -> &'static RuleMeta {
        &MAX_NESTED_FUNCTIONS
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::Function, AstType::ArrowFunctionExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        // Functions arrive in source pre-order: a stack of the open ones (those whose span has not
        // ended yet) holds exactly the enclosing functions.
        let span = node.span();
        let counted = counts(ctx, node);
        let depth = {
            let open = &mut ctx.state::<OpenFunctions>().stack;
            while open.last().is_some_and(|&(end, _)| end <= span.start) {
                open.pop();
            }
            let depth = open.iter().filter(|&&(_, c)| c).count();
            open.push((span.end, counted));
            depth
        };
        if !counted {
            return;
        }
        let max = match ctx.state::<OpenFunctions>().max {
            Some(max) => max,
            None => {
                let max = ctx.param_i64("max").max(1) as usize;
                ctx.state::<OpenFunctions>().max = Some(max);
                max
            }
        };
        // Report only the first level past the limit.
        if depth != max {
            return;
        }
        let span = match node.kind() {
            AstKind::Function(f) => match &f.id {
                Some(id) => id.span,
                None => util::first_line(ctx, f.span),
            },
            AstKind::ArrowFunctionExpression(a) => util::first_line(ctx, a.span),
            _ => return,
        };
        let span = Span::new(span.start, span.end.min(span.start + 80));
        ctx.emit(
            span,
            format!(
                "Move this function out: it is nested {} levels deep (at most {max} allowed)",
                depth + 1
            ),
        )
        .value((depth + 1) as f64);
    }
}

/// The root identifier of a callee chain (`describe.each(t)` → `describe`) and whether the callee
/// is that identifier alone. No allocation: this runs for every callback.
fn callee_root<'b, 'a>(callee: &'b Expression<'a>) -> Option<(&'b IdentifierReference<'a>, bool)> {
    let mut current = helpers::unwrap_expr(callee);
    let mut plain = true;
    loop {
        match current {
            Expression::Identifier(id) => return Some((id, plain)),
            Expression::StaticMemberExpression(m) => current = helpers::unwrap_expr(&m.object),
            Expression::CallExpression(c) => current = helpers::unwrap_expr(&c.callee),
            _ => return None,
        }
        plain = false;
    }
}

/// The functions enclosing the current one: (span end, adds a level).
#[derive(Default)]
struct OpenFunctions {
    stack: Vec<(u32, bool)>,
    /// The `max` parameter, read once per file.
    max: Option<usize>,
}

/// Does this function add a level of nesting?
fn counts(ctx: &Ctx<'_>, node: &AstNode<'_>) -> bool {
    // A one-expression arrow (`x => x.id`) opens no block and no indentation level.
    if matches!(node.kind(), AstKind::ArrowFunctionExpression(a) if a.expression) {
        return false;
    }
    let Some(parent) = ctx.outer_parent(node) else {
        return true;
    };
    match parent.kind() {
        // `(function () { … })()`
        AstKind::CallExpression(call)
            if helpers::unwrap_expr(&call.callee).span() == node.span() =>
        {
            false
        }
        // `(function (factory) { … }(function ($) { … }))`: a module factory handed to a wrapper
        // called on the spot (UMD).
        AstKind::CallExpression(call)
            if matches!(
                helpers::unwrap_expr(&call.callee),
                Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_)
            ) =>
        {
            false
        }
        AstKind::CallExpression(call) => {
            match callee_root(&call.callee) {
                Some((root, plain)) => {
                    // AMD module factories.
                    if plain && root.name == "define" && ctx.is_global_reference(root) {
                        return false;
                    }
                    // Test suites and cases.
                    !(ctx.is_test()
                        && matches!(
                            root.name.as_str(),
                            "describe"
                                | "context"
                                | "suite"
                                | "it"
                                | "test"
                                | "specify"
                                | "beforeEach"
                                | "afterEach"
                                | "beforeAll"
                                | "afterAll"
                                | "before"
                                | "after"
                                | "setup"
                                | "teardown"
                                | "suiteSetup"
                                | "suiteTeardown"
                                | "xit"
                                | "xdescribe"
                                | "fit"
                                | "fdescribe"
                        )
                        && util::is_test_api(ctx, root))
                }
                None => true,
            }
        }
        _ => true,
    }
}
