//! `js/no-for-in-array`

use super::util::is_visible_array;
use crate::prelude::*;

pub struct NoForInArray;

pub static NO_FOR_IN_ARRAY: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["suspicious"],
    remediation_minutes: 2,
    description: r#"Flags `for (… in …)` loops over an array: an array literal, `Object.keys(…)` /
`Object.values(…)` / `Object.entries(…)`, `Array.from(…)`, or a variable declared or initialized as
an array.

**Why:** `for…in` walks the *property names* of an object, as strings. Over an array it yields
`'0'`, `'1'`, … instead of the elements — `for (const key in Object.keys(obj))` gives the indexes
of the key list, not the keys — plus any enumerable property added to the array or to
`Array.prototype`, and skips the holes of sparse arrays. Index arithmetic on the string keys
(`key + 1`) concatenates instead of adding.

**Fix:** use `for…of` for the elements (`for (const key of Object.keys(obj))`), `forEach`, or a
counting loop for the indexes.

```js
for (const key in Object.keys(options)) {}  // flagged: '0', '1', …
for (const i in [1, 2, 3]) {}               // flagged
for (const key of Object.keys(options)) {}  // compliant
for (const key in options) {}               // compliant: an object
```

Arrays are only recognized when that is visible in the file: without type information, a `for…in`
over a parameter or a call result is not reported. A loop whose key is only used to index the same
array (`for (const i in list) use(list[i])`) works despite the string keys and is not reported
either.
"#,
    ..RuleMeta::new(
        "js/no-for-in-array",
        "js",
        "Arrays should not be iterated with 'for…in'",
    )
};

impl JsRule for NoForInArray {
    fn meta(&self) -> &'static RuleMeta {
        &NO_FOR_IN_ARRAY
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::ForInStatement]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::ForInStatement(stmt) = node.kind() else {
            return;
        };
        if !is_visible_array(ctx, &stmt.right) {
            return;
        }
        if only_indexes_the_array(ctx, stmt) {
            return;
        }
        let what = match helpers::unwrap_expr(&stmt.right) {
            Expression::CallExpression(call)
                if matches!(
                    helpers::call_member_names(call),
                    Some(("Object", "keys" | "values" | "entries"))
                ) =>
            {
                "an array returned by Object.keys/values/entries"
            }
            _ => "an array",
        };
        let right = stmt.right.span();
        ctx.emit(
            Span::new(stmt.span.start, right.end),
            format!("This 'for…in' walks the indexes of {what} as strings, not its elements: use 'for…of' instead"),
        );
    }
}

/// `for (const i in list) { use(list[i]); }`: the key is only ever used to index the same array
/// (or not used at all), which works — the string indexes are converted back. Only a key used as
/// something else (compared, passed on, used on another object) shows the mix-up.
fn only_indexes_the_array(ctx: &Ctx<'_>, stmt: &ForInStatement<'_>) -> bool {
    let key = match &stmt.left {
        ForStatementLeft::VariableDeclaration(decl) => {
            match decl.declarations.first().map(|d| &d.id) {
                Some(BindingPattern::BindingIdentifier(id)) => ctx.binding_symbol(id),
                _ => None,
            }
        }
        ForStatementLeft::AssignmentTargetIdentifier(id) => ctx.reference_symbol(id),
        _ => None,
    };
    let Some(key) = key else {
        return false;
    };
    let array = match helpers::unwrap_expr(&stmt.right) {
        Expression::Identifier(id) => ctx.reference_symbol(id),
        _ => None,
    };
    ctx.references_of(key).all(|r| {
        if !r.is_read() {
            return true;
        }
        let node = ctx.node(r.node_id());
        let Some(AstKind::ComputedMemberExpression(m)) = ctx.parent_kind(node) else {
            return false;
        };
        m.expression.span() == node.span()
            && array.is_some()
            && matches!(helpers::unwrap_expr(&m.object), Expression::Identifier(o) if ctx.reference_symbol(o) == array)
    })
}
