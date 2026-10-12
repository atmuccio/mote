//! `js/no-array-constructor`

use crate::prelude::*;

pub struct NoArrayConstructor;

pub static NO_ARRAY_CONSTRUCTOR: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["convention"],
    remediation_minutes: 1,
    description: r#"Flags `Array(…)` and `new Array(…)` called with zero arguments or with two or more
arguments.

**Why:** the `Array` constructor is ambiguous: with one numeric argument it creates an empty
array of that length, with several arguments it creates an array of those elements. An array
literal always means "these elements" and is shorter.

**Fix:** use an array literal: `[]`, `[a, b]`. A single argument (`new Array(n)`, a pre-sized
array) and calls with TypeScript type arguments (`new Array<Item>()`) are not reported.

```js
const empty = new Array();      // flagged
const pair = Array(x, y);       // flagged

const empty = [];               // compliant
const pair = [x, y];            // compliant
const slots = new Array(size);  // compliant
```
"#,
    ..RuleMeta::new(
        "js/no-array-constructor",
        "js",
        "Array literals should be used instead of the 'Array' constructor",
    )
};

impl JsRule for NoArrayConstructor {
    fn meta(&self) -> &'static RuleMeta {
        &NO_ARRAY_CONSTRUCTOR
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::NewExpression, AstType::CallExpression]
    }

    fn callee_names(&self) -> &'static [&'static str] {
        &["Array"]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let (callee, arguments, type_arguments, span) = match node.kind() {
            AstKind::NewExpression(n) => (&n.callee, &n.arguments, &n.type_arguments, n.span),
            AstKind::CallExpression(c) if !c.optional => {
                (&c.callee, &c.arguments, &c.type_arguments, c.span)
            }
            _ => return,
        };
        if arguments.len() == 1 || type_arguments.is_some() {
            return;
        }
        if !ctx.is_global(callee, "Array") {
            return;
        }
        // `Array` wrapped in `as`/`!` is unusual; only the plain global identifier counts.
        if !matches!(callee, Expression::Identifier(_)) {
            return;
        }
        ctx.emit(
            span,
            "Use an array literal instead of the 'Array' constructor",
        );
    }
}
