//! `js/consistent-return`

use super::util;
use crate::prelude::*;

pub struct ConsistentReturn;

pub static CONSISTENT_RETURN: RuleMeta = RuleMeta {
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["confusing", "suspicious"],
    remediation_minutes: 5,
    description: r#"Flags a predicate, a function whose `return` statements give booleans (`true`, `false`, a
comparison, a negation), that also uses a bare `return;`, which gives `undefined`.

**Why:** the function answers a yes/no question on every path but one, where it answers nothing.
`undefined` happens to be falsy, so a plain `if (isValid(x))` still works, but any caller that
compares the answer (`=== false`), stores it, serializes it or counts it gets a third value that
nobody meant to produce. A bare `return` in a predicate is almost always an early exit written
before the function started returning booleans.

**Fix:** return the boolean that the early exit means, usually `return false;`.

```js
function isEditable(doc) {
  if (!doc) {
    return;                          // flagged: undefined instead of false
  }
  return !doc.readOnly && doc.owner === user;
}

function isEditable(doc) {
  if (!doc) {
    return false;                    // compliant
  }
  return !doc.readOnly && doc.owner === user;
}
```

Only predicates are checked: in other functions a bare `return` often means "no result" on
purpose, which the rule cannot tell from a slip. Not reported either: functions that also return
`undefined` explicitly (a deliberate third state), functions passed directly as arguments
(callbacks, whose callers define what a missing value means), getters, setters, constructors,
generators, and TypeScript functions whose declared return type includes `undefined`, `void`,
`any` or `unknown`.
"#,
    ..RuleMeta::new(
        "js/consistent-return",
        "js",
        "A function should either always or never return a value",
    )
};

impl JsRule for ConsistentReturn {
    fn meta(&self) -> &'static RuleMeta {
        &CONSISTENT_RETURN
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::ReturnStatement]
    }

    /// Cheap pass: remember the functions that contain a bare `return;`.
    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::ReturnStatement(ret) = node.kind() else {
            return;
        };
        if ret.argument.is_some() {
            return;
        }
        let Some(function) = ctx.enclosing_function(node).map(AstNode::id) else {
            return;
        };
        let state = ctx.state::<WithBareReturn>();
        if state.functions.last() != Some(&function) && !state.functions.contains(&function) {
            state.functions.push(function);
        }
    }

    fn run_once<'a>(&self, ctx: &mut Ctx<'a>) {
        let functions = std::mem::take(&mut ctx.state::<WithBareReturn>().functions);
        for id in functions {
            let node = ctx.node(id);
            check_function(node, ctx);
        }
    }
}

/// Functions holding a bare `return;`, in the order first seen.
#[derive(Default)]
struct WithBareReturn {
    functions: Vec<NodeId>,
}

fn check_function<'a>(node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
    let kind = node.kind();
    let return_type = match kind {
        AstKind::Function(f) => {
            if f.generator || f.declare || f.body.is_none() {
                return;
            }
            f.return_type.as_deref()
        }
        AstKind::ArrowFunctionExpression(a) => {
            if a.expression {
                return;
            }
            a.return_type.as_deref()
        }
        _ => return,
    };
    if let Some(annotation) = return_type {
        if allows_undefined(ctx.text(annotation.span)) {
            return;
        }
    }
    match ctx.outer_parent(node).map(AstNode::kind) {
        Some(AstKind::MethodDefinition(m)) if m.kind != MethodDefinitionKind::Method => return,
        Some(AstKind::ObjectProperty(p)) if p.kind != PropertyKind::Init => return,
        Some(AstKind::CallExpression(_) | AstKind::NewExpression(_)) => {
            return;
        }
        _ => {}
    }
    let mut bare: Vec<Span> = Vec::new();
    let mut values: Vec<&ReturnStatement<'a>> = Vec::new();
    for inner in util::own_nodes(ctx, node) {
        let AstKind::ReturnStatement(ret) = inner.kind() else {
            continue;
        };
        match &ret.argument {
            None => bare.push(ret.span),
            Some(_) => values.push(ret),
        }
    }
    if bare.is_empty() {
        return;
    }
    // Only predicates: every value returned must be a boolean. Other functions often use a
    // bare `return` to mean "no result" on purpose.
    if !values
        .iter()
        .all(|r| r.argument.as_ref().is_some_and(is_boolean))
    {
        return;
    }
    let valued = values.first().map(|r| r.span);
    let (Some(valued), Some(&first)) = (valued, bare.first()) else {
        return;
    };
    let valued = util::first_line(ctx, valued);
    let mut emitted = ctx
            .emit(
                first,
                "Return false here: the other paths of this function return a boolean, this one undefined",
            )
            .secondary(valued, "Returns a boolean");
    for &span in &bare[1..] {
        emitted = emitted.secondary(span, "Also returns nothing");
    }
}

/// Is the expression certainly a boolean: a literal, a negation, a comparison, `instanceof`, `in`,
/// or a combination of those with `&&`/`||`?
fn is_boolean(expr: &Expression<'_>) -> bool {
    // An explicit stack: long `a && b && …` chains must not recurse.
    let mut pending: Vec<&Expression<'_>> = vec![expr];
    while let Some(expr) = pending.pop() {
        let ok = match helpers::unwrap_expr(expr) {
            Expression::BooleanLiteral(_) | Expression::PrivateInExpression(_) => true,
            Expression::UnaryExpression(u) => u.operator == UnaryOperator::LogicalNot,
            Expression::BinaryExpression(b) => {
                b.operator.is_equality()
                    || b.operator.is_compare()
                    || matches!(b.operator, BinaryOperator::Instanceof | BinaryOperator::In)
            }
            Expression::LogicalExpression(l) if l.operator != LogicalOperator::Coalesce => {
                pending.push(&l.left);
                pending.push(&l.right);
                true
            }
            _ => false,
        };
        if !ok {
            return false;
        }
    }
    true
}

/// Does a return type annotation admit `undefined` (`void`, `undefined`, `any`, `unknown`,
/// possibly inside `Promise<…>` or a union)?
fn allows_undefined(annotation: &str) -> bool {
    annotation
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '$'))
        .any(|word| matches!(word, "void" | "undefined" | "any" | "unknown"))
}
