//! `js/no-sparse-arrays`

use crate::prelude::*;

pub struct NoSparseArrays;

pub static NO_SPARSE_ARRAYS: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["suspicious"],
    remediation_minutes: 2,
    description: r#"Flags array literals with holes — consecutive commas such as `[1, , 3]` or a leading
comma `[, a]`.

**Why:** a hole is not `undefined`: it is a missing element that many array methods (`forEach`,
`map`, `filter`, spreading into function arguments) treat differently. Holes are also easy to miss
when reading, so a double comma is usually a typo — an element was deleted or never written.

**Fix:** remove the extra comma, or write `undefined` explicitly if an empty slot is wanted.

```js
const colors = ['red', , 'blue'];          // flagged
const colors = ['red', 'blue'];            // compliant
const slots = ['red', undefined, 'blue'];  // compliant: explicit
```

A trailing comma (`[1, 2,]`) is not a hole, and holes in destructuring patterns
(`const [, second] = pair`) are fine.
"#,
    ..RuleMeta::new(
        "js/no-sparse-arrays",
        "js",
        "Array literals should not contain holes",
    )
};

impl JsRule for NoSparseArrays {
    fn meta(&self) -> &'static RuleMeta {
        &NO_SPARSE_ARRAYS
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::ArrayExpression]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.file_contains(AstType::Elision)
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::ArrayExpression(array) = node.kind() else {
            return;
        };
        let Some(hole) = array.elements.iter().find_map(|e| match e {
            ArrayExpressionElement::Elision(h) => Some(h.span),
            _ => None,
        }) else {
            return;
        };
        ctx.emit(
            hole,
            "Remove the hole in this array literal, or write 'undefined' if an empty slot is intended",
        );
    }
}
