//! `js/in-operator-misuse`

use super::util::is_visible_array;
use crate::prelude::*;

pub struct InOperatorMisuse;

pub static IN_OPERATOR_MISUSE: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["suspicious"],
    remediation_minutes: 2,
    description: r#"Flags the `in` operator used on a primitive value (`'a' in 'abc'`) or used to look for a value in
an array (`'admin' in ['admin', 'owner']`).

**Why:** `in` tests whether an *object* has a *property* with that name. On a string, number or
boolean it throws a `TypeError` at run time. On an array, it checks the indexes (`'0'`, `'1'`, …)
and `length`, never the elements, so `'admin' in ['admin', 'owner']` is `false`.

**Fix:** use `includes` for membership — `['admin', 'owner'].includes(role)`,
`'abc'.includes('a')` — or a `Set`.

```js
if ('admin' in ['admin', 'owner']) {}      // flagged: always false
if ('x' in 'xyz') {}                       // flagged: throws a TypeError
if (['admin', 'owner'].includes(role)) {}  // compliant
if ('id' in record) {}                     // compliant: property test on an object
if (0 in sparse) {}                        // compliant: index test on an array
```

Arrays count when they are visible: literals, `Object.keys(…)`, `Array.from(…)`, or variables
declared or initialized as arrays. On arrays, only string literals that are not array indexes are
reported; a numeric or unknown key may be a deliberate index test.
"#,
    ..RuleMeta::new(
        "js/in-operator-misuse",
        "js",
        "The 'in' operator should not be used on primitives or to search arrays",
    )
};

impl JsRule for InOperatorMisuse {
    fn meta(&self) -> &'static RuleMeta {
        &IN_OPERATOR_MISUSE
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::BinaryExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::BinaryExpression(expr) = node.kind() else {
            return;
        };
        if expr.operator != BinaryOperator::In {
            return;
        }
        let right = helpers::unwrap_expr(&expr.right);
        let primitive = match right {
            Expression::StringLiteral(_) | Expression::TemplateLiteral(_) => Some("a string"),
            Expression::NumericLiteral(_) | Expression::BigIntLiteral(_) => Some("a number"),
            Expression::BooleanLiteral(_) => Some("a boolean"),
            Expression::NullLiteral(_) => Some("null"),
            _ if right.is_undefined() && ctx.is_global(right, "undefined") => Some("undefined"),
            _ => None,
        };
        if let Some(what) = primitive {
            ctx.emit(
                expr.span,
                format!("'in' on {what} throws a TypeError: 'in' tests the properties of objects; use 'includes' to search a string"),
            );
            return;
        }
        let Some(key) = helpers::string_value(helpers::unwrap_expr(&expr.left)) else {
            return;
        };
        // `'flat' in []` detects a feature of arrays.
        if is_array_index(key) || ARRAY_PROPERTIES.contains(&key) {
            return;
        }
        if !is_visible_array(ctx, right) {
            return;
        }
        // An array used as an object (`list.extra = 1`) can have other properties.
        if let Expression::Identifier(id) = right {
            if ctx.reference_symbol(id).is_none_or(|s| {
                ctx.references_of(s)
                    .any(|r| r.flags().contains(ReferenceFlags::MemberWriteTarget))
            }) {
                return;
            }
        }
        ctx.emit(
            expr.span,
            format!("'{key}' in an array tests the array's indexes, not its values, so this is always false: use 'includes' instead"),
        );
    }
}

/// Properties every array has (own, from `Array.prototype`, or from `Object.prototype`).
const ARRAY_PROPERTIES: &[&str] = &[
    "length",
    "at",
    "concat",
    "copyWithin",
    "entries",
    "every",
    "fill",
    "filter",
    "find",
    "findIndex",
    "findLast",
    "findLastIndex",
    "flat",
    "flatMap",
    "forEach",
    "includes",
    "indexOf",
    "join",
    "keys",
    "lastIndexOf",
    "map",
    "pop",
    "push",
    "reduce",
    "reduceRight",
    "reverse",
    "shift",
    "slice",
    "some",
    "sort",
    "splice",
    "toLocaleString",
    "toReversed",
    "toSorted",
    "toSpliced",
    "toString",
    "unshift",
    "values",
    "with",
    "constructor",
    "hasOwnProperty",
    "isPrototypeOf",
    "propertyIsEnumerable",
    "valueOf",
    "__proto__",
    "__defineGetter__",
    "__defineSetter__",
    "__lookupGetter__",
    "__lookupSetter__",
];

/// `0`, `1`, `42` — canonical array index strings.
fn is_array_index(key: &str) -> bool {
    !key.is_empty()
        && key.bytes().all(|b| b.is_ascii_digit())
        && (key == "0" || !key.starts_with('0'))
}
