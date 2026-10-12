//! `java/simplifiable-ternary` — `c ? true : x` is `c || x`.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["clutter", "readability"],
    remediation_minutes: 1,
    description: r#"Detects conditional expressions with one boolean-literal branch and one boolean
branch, which are just `&&` / `||` in disguise:

| Conditional       | Same as      |
|-------------------|--------------|
| `c ? true : x`    | `c \|\| x`   |
| `c ? false : x`   | `!c && x`    |
| `c ? x : true`    | `!c \|\| x`  |
| `c ? x : false`   | `c && x`     |

**Why it matters.** The logical operators say what is meant in fewer symbols; a conditional
with a literal branch makes readers evaluate both paths in their head.

**How to fix.** Rewrite with `&&` / `||` (negating the condition where needed).

```java
// Noncompliant
boolean visible = isAdmin ? true : item.isPublic();

// Compliant
boolean visible = isAdmin || item.isPublic();
```

The other branch must be known to be boolean: a comparison, a logical expression, a
`boolean`/`Boolean` value, or any expression when the context needs a `boolean` (the conditional
initializes or is assigned to a `boolean` variable, is returned from a method declared
`boolean`, or is an operand of `&&`, `||` or `!`), since only a boolean branch compiles there.
`c ? true : false` is reported by `java/boolean-literal-comparison` instead."#,
    ..RuleMeta::new(
        "java/simplifiable-ternary",
        "java",
        "Conditional expression with a boolean literal branch",
    )
};

pub struct SimplifiableTernary;

/// Syntactically boolean expressions (no type information needed).
fn is_boolean_shape(n: Node<'_>) -> bool {
    let n = syntax::strip_parens(n);
    match kind(n) {
        "binary_expression" => matches!(
            syntax::operator_str(n),
            "==" | "!=" | "<" | ">" | "<=" | ">=" | "&&" | "||"
        ),
        "unary_expression" => syntax::operator_str(n) == "!",
        "instanceof_expression" => true,
        _ => false,
    }
}

/// Whether the conditional sits where only a `boolean` compiles: the value of a `boolean`
/// variable or of an assignment to one, the result of a method declared `boolean`, or an operand
/// of a logical operator.
fn boolean_context<'t>(ctx: &Ctx<'t>, node: Node<'t>) -> bool {
    let mut child = node;
    let mut parent = ctx.parent(node);
    while let Some(p) = parent.filter(|p| kind(*p) == "parenthesized_expression") {
        child = p;
        parent = ctx.parent(p);
    }
    let Some(p) = parent else {
        return false;
    };
    let is_boolean_type = |t: Option<Node<'t>>| t.is_some_and(|t| ctx.text(t) == "boolean");
    match kind(p) {
        "binary_expression" => matches!(syntax::operator_str(p), "&&" | "||"),
        "unary_expression" => syntax::operator_str(p) == "!",
        "variable_declarator" => {
            util::fld(p, F.value).is_some_and(|v| v.id() == child.id())
                && util::fld(p, F.dimensions).is_none()
                && is_boolean_type(ctx.parent(p).and_then(|d| util::fld(d, F.ty)))
        }
        "assignment_expression" => {
            syntax::operator_str(p) == "="
                && util::fld(p, F.right).is_some_and(|r| r.id() == child.id())
                && util::fld(p, F.left)
                    .filter(|l| kind(*l) == "identifier")
                    .and_then(|l| ctx.declaration_of(l))
                    .is_some_and(|d| d.dims == 0 && is_boolean_type(d.type_node))
        }
        "return_statement" => ctx
            .enclosing_function(p)
            .filter(|f| kind(*f) == "method_declaration")
            .is_some_and(|m| {
                util::fld(m, F.dimensions).is_none() && is_boolean_type(util::fld(m, F.ty))
            }),
        _ => false,
    }
}

impl<'t> JavaRule<'t> for SimplifiableTernary {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["ternary_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let (Some(a), Some(b)) = (
            util::fld(node, F.consequence),
            util::fld(node, F.alternative),
        ) else {
            return;
        };
        let (lit, other, lit_first) = match (util::bool_literal(a), util::bool_literal(b)) {
            (Some(v), None) => (v, b, true),
            (None, Some(v)) => (v, a, false),
            _ => return,
        };
        if !is_boolean_shape(other)
            && !ctx.type_of(other).is_some_and(|t| t.is_boolean_like())
            && !boolean_context(ctx, node)
        {
            return;
        }
        let replacement = match (lit_first, lit) {
            (true, true) => "cond || other",
            (true, false) => "!cond && other",
            (false, true) => "!cond || other",
            (false, false) => "cond && other",
        };
        ctx.emit(
            node,
            format!("Replace this conditional with `{replacement}`"),
        );
    }
}
