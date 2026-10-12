//! `js/top-level-this`

use crate::prelude::*;
use crate::rules::batch_6::util::program;

pub struct TopLevelThis;

pub static TOP_LEVEL_THIS: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["suspicious", "modules"],
    remediation_minutes: 5,
    description: r#"Flags `this` at the top level of an ES module: outside every function and class body,
including inside arrow functions at the top level, which take `this` from the module.

**Why:** at the top level of an ES module `this` is `undefined`, so `this.x` throws and `this`
passed along is a surprise. (A build that turns the module into CommonJS binds it to
`module.exports` instead, which only works until the code runs as a real module or goes through
a bundler.) The code was usually moved out of a class or a function, or written for an
old-style script where the top-level `this` was the global object; `globalThis` names that object
explicitly.

**Fix:** use `globalThis` (or `window`) for the global object, or the object that was meant.

```js
import { render } from './render.js';
this.app = render();          // flagged: this is undefined in a module
const onClick = () => this.toggle();   // flagged

globalThis.app = render();    // compliant
```

Only files that are ES modules by their syntax (with `import` or `export`, or a `.mjs`/`.mts`
extension, and without the CommonJS-only `export =` or `import x = require(…)`) are checked: in a
script or a CommonJS file the top-level `this` depends on how the file is loaded. Guards that test `this` before using it (`(this && this.helper) || …`, `typeof this`), as emitted
by compilers, are not reported, nor is `this` handed to a wrapper function that inspects its
environment (`(function (root) { … })(this)`, as in UMD bundles), nor `this` inside TypeScript
namespaces.
"#,
    ..RuleMeta::new(
        "js/top-level-this",
        "js",
        "'this' should not be used at the top level of a module",
    )
};

/// Is this expression `this` (through TypeScript wrappers)?
fn is_this(expr: &Expression<'_>) -> bool {
    matches!(helpers::unwrap_expr(expr), Expression::ThisExpression(_))
}

impl JsRule for TopLevelThis {
    fn meta(&self) -> &'static RuleMeta {
        &TOP_LEVEL_THIS
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::ThisExpression]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        if !ctx.is_module() || ctx.source_type().is_commonjs() {
            return false;
        }
        let Some(program) = program(ctx) else {
            return false;
        };
        // `export = x` and `import x = require('…')` only exist for CommonJS output, where the
        // top-level `this` is `module.exports`.
        if program.body.iter().any(|s| {
            matches!(
                s,
                Statement::TSExportAssignment(_) | Statement::TSImportEqualsDeclaration(_)
            )
        }) {
            return false;
        }
        // Explicit module syntax: a fragment that is a module only through a top-level `await`
        // (a snippet cut out of a method, kept as test data) says nothing about its `this`.
        let path = ctx.path();
        path.ends_with(".mjs")
            || path.ends_with(".mts")
            || program.body.iter().any(|s| {
                matches!(
                    s,
                    Statement::ImportDeclaration(_)
                        | Statement::ExportNamedDeclaration(_)
                        | Statement::ExportDefaultDeclaration(_)
                        | Statement::ExportAllDeclaration(_)
                )
            })
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::ThisExpression(this) = node.kind() else {
            return;
        };
        let at = this.span;
        for ancestor in ctx.ancestors(node) {
            match ancestor.kind() {
                // A function or class body gives `this` its own value.
                AstKind::Function(_)
                | AstKind::StaticBlock(_)
                | AstKind::TSModuleDeclaration(_) => {
                    return;
                }
                AstKind::PropertyDefinition(p) => {
                    if p.value
                        .as_ref()
                        .is_some_and(|v| v.span().contains_inclusive(at))
                    {
                        return;
                    }
                }
                AstKind::AccessorProperty(p) => {
                    if p.value
                        .as_ref()
                        .is_some_and(|v| v.span().contains_inclusive(at))
                    {
                        return;
                    }
                }
                // `(this && this.helper) || …`, `typeof this`, `this ? … : …`,
                // `this === undefined`: the code tests `this` before relying on it.
                AstKind::LogicalExpression(l) if is_this(&l.left) => return,
                AstKind::ConditionalExpression(c) if is_this(&c.test) => return,
                AstKind::UnaryExpression(u) if u.operator == UnaryOperator::Typeof => return,
                AstKind::BinaryExpression(b)
                    if b.operator.is_equality() && (is_this(&b.left) || is_this(&b.right)) =>
                {
                    return;
                }
                // `(function (root, factory) { … })(this, …)` and
                // `(function (g) { … })(typeof self !== 'undefined' ? self : this)`: a wrapper (UMD)
                // that inspects the environment it is handed.
                AstKind::CallExpression(call)
                    if matches!(
                        helpers::unwrap_expr(&call.callee),
                        Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_)
                    ) && call
                        .arguments
                        .iter()
                        .any(|a| a.span().contains_inclusive(at)) =>
                {
                    return;
                }
                AstKind::Program(_) => break,
                _ => {}
            }
        }
        ctx.emit(
            at,
            "Replace 'this' at the top level of a module: in an ES module it is undefined (use 'globalThis' for the global object)",
        );
    }
}
