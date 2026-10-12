//! `js/overwritten-before-read`

use super::util;
use crate::prelude::*;
use oxc_span::ContentEq;

pub struct OverwrittenBeforeRead;

pub static OVERWRITTEN_BEFORE_READ: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["suspicious", "copy-paste"],
    remediation_minutes: 5,
    description: r#"Flags a property or map entry that is written and then, a statement or two later,
written again before anything could read it: `obj.a = 1; obj.a = 2;` or
`map.set('k', a); map.set('k', b);`.

**Why:** the first value is lost. This is nearly always a copy-paste mistake: the second statement
was meant to target another property or key, or the first one is left over from an edit.

**Fix:** change the target of one of the writes, or delete the first one.

```js
style.width = w;
style.width = h;              // flagged on the first line: `style.height` was meant

headers.set('Accept', json);
headers.set('Accept', text);  // flagged on the first line

style.width = w;              // compliant
style.height = h;
```

Only straight-line code is considered: the writes must be in consecutive statements (other
assignments to different properties of the same object may sit between them), and the second value
must be simple — no calls, `new`, `await`, or assignments, and no mention of the object — so that
nothing can observe the first value in between. Not reported: writes to `this` in classes that
declare setters, a property first set to a constant (`el.hidden = true; el.hidden = false;` is a
common way to poke a setter or reset state), writes next to an explanatory comment,
and test files, which often assign twice on purpose to exercise setters.
"#,
    ..RuleMeta::new(
        "js/overwritten-before-read",
        "js",
        "Values should not be overwritten before they are read",
    )
};

/// How many statements may separate the two writes.
const WINDOW: usize = 4;

impl JsRule for OverwrittenBeforeRead {
    fn meta(&self) -> &'static RuleMeta {
        &OVERWRITTEN_BEFORE_READ
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        // Tests assign twice on purpose to exercise setters and overwrite semantics.
        !ctx.is_test()
    }

    fn types(&self) -> &'static [AstType] {
        &[
            AstType::Program,
            AstType::FunctionBody,
            AstType::BlockStatement,
            AstType::SwitchCase,
            AstType::StaticBlock,
        ]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let list: &'a [Statement<'a>] = match node.kind() {
            AstKind::Program(p) => &p.body,
            AstKind::FunctionBody(b) => &b.statements,
            AstKind::BlockStatement(b) => &b.body,
            AstKind::SwitchCase(c) => &c.consequent,
            AstKind::StaticBlock(b) => &b.body,
            _ => return,
        };
        if list.len() < 2 {
            return;
        }
        let mut setters_checked: Option<bool> = None;
        for (i, stmt) in list.iter().enumerate() {
            let Some(first) = write_of(stmt) else {
                continue;
            };
            // Toggling or resetting a property through a constant is usually done on purpose (to
            // trigger a setter, re-run an animation, or force a change event).
            if matches!(first.slot, Slot::Property(_)) && first.value.is_some_and(is_constant) {
                continue;
            }
            for next in list.iter().skip(i + 1).take(WINDOW) {
                let Some(second) = write_of(next) else {
                    break;
                };
                if !second.value_is_simple(&first.root) {
                    break;
                }
                if second.same_slot(&first) {
                    if matches!(first.root, Root::This)
                        && *setters_checked.get_or_insert_with(|| class_has_setters(ctx, node))
                    {
                        break;
                    }
                    // A comment next to the writes usually explains why both are needed.
                    if comment_just_before(ctx, stmt.span().start)
                        || own_line_comment_between(ctx, stmt.span().end, next.span().start)
                    {
                        break;
                    }
                    report(ctx, &first, &second);
                    break;
                }
                // Another property of the same object, on a path that neither passes through
                // nor replaces the first target: keep looking.
                if !second.root.same(&first.root) || !diverges(&first, &second) {
                    break;
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Root<'a> {
    This,
    Ident(&'a str),
}

impl Root<'_> {
    fn same(&self, other: &Root<'_>) -> bool {
        match (self, other) {
            (Root::This, Root::This) => true,
            (Root::Ident(a), Root::Ident(b)) => a == b,
            _ => false,
        }
    }
}

enum Slot<'a> {
    /// `target = value`
    Property(&'a SimpleAssignmentTarget<'a>),
    /// `receiver.set(key, value)`
    Entry {
        receiver: &'a Expression<'a>,
        key: &'a Expression<'a>,
    },
}

struct Write<'a> {
    slot: Slot<'a>,
    root: Root<'a>,
    /// Everything evaluated besides the target: the assigned value, or the call arguments.
    values: &'a [Argument<'a>],
    value: Option<&'a Expression<'a>>,
    span: Span,
    target_span: Span,
}

impl<'a> Write<'a> {
    fn same_slot(&self, other: &Write<'_>) -> bool {
        match (&self.slot, &other.slot) {
            (Slot::Property(a), Slot::Property(b)) => a.content_eq(b),
            (
                Slot::Entry {
                    receiver: ra,
                    key: ka,
                },
                Slot::Entry {
                    receiver: rb,
                    key: kb,
                },
            ) => ra.content_eq(rb) && same_key(ka, kb),
            _ => false,
        }
    }

    /// Could evaluating this write observe the previous value of `root`'s properties?
    fn value_is_simple(&self, root: &Root<'_>) -> bool {
        let value_ok = self.value.is_none_or(|v| is_simple(v, root));
        let args_ok = self
            .values
            .iter()
            .all(|a| a.as_expression().is_some_and(|e| is_simple(e, root)));
        value_ok && args_ok
    }
}

/// Two property writes whose paths from the common root split before either ends: `a.x.y` and
/// `a.z` diverge; `a.x` and `a.x.y` do not (the second reads or replaces the first's object).
fn diverges(first: &Write<'_>, second: &Write<'_>) -> bool {
    let (Slot::Property(a), Slot::Property(b)) = (&first.slot, &second.slot) else {
        return false;
    };
    let (mut pa, mut pb) = (Vec::new(), Vec::new());
    if !target_path(a, &mut pa) || !target_path(b, &mut pb) {
        return false;
    }
    pa.iter().zip(&pb).any(|(x, y)| x != y)
}

/// Property names from the root outwards (`this.a['b'].#c` → `a`, `b`, `#c`).
fn target_path<'a>(target: &SimpleAssignmentTarget<'a>, out: &mut Vec<&'a str>) -> bool {
    let (object, last): (&Expression<'a>, &'a str) = match target {
        SimpleAssignmentTarget::StaticMemberExpression(m) => (&m.object, m.property.name.as_str()),
        SimpleAssignmentTarget::PrivateFieldExpression(m) => (&m.object, m.field.name.as_str()),
        SimpleAssignmentTarget::ComputedMemberExpression(m) => match &m.expression {
            Expression::StringLiteral(s) => (&m.object, s.value.as_str()),
            Expression::NumericLiteral(n) => match n.raw {
                Some(raw) => (&m.object, raw.as_str()),
                None => return false,
            },
            _ => return false,
        },
        _ => return false,
    };
    if !object_path(object, out) {
        return false;
    }
    out.push(last);
    true
}

fn object_path<'a>(expr: &Expression<'a>, out: &mut Vec<&'a str>) -> bool {
    match expr {
        Expression::Identifier(_) | Expression::ThisExpression(_) => true,
        Expression::StaticMemberExpression(m) => {
            if !object_path(&m.object, out) {
                return false;
            }
            out.push(m.property.name.as_str());
            true
        }
        Expression::PrivateFieldExpression(m) => {
            if !object_path(&m.object, out) {
                return false;
            }
            out.push(m.field.name.as_str());
            true
        }
        _ => false,
    }
}

fn same_key(a: &Expression<'_>, b: &Expression<'_>) -> bool {
    match (helpers::unwrap_expr(a), helpers::unwrap_expr(b)) {
        (Expression::StringLiteral(x), Expression::StringLiteral(y)) => x.value == y.value,
        (Expression::NumericLiteral(x), Expression::NumericLiteral(y)) => x.value == y.value,
        _ => false,
    }
}

fn write_of<'a>(stmt: &'a Statement<'a>) -> Option<Write<'a>> {
    let Statement::ExpressionStatement(stmt) = stmt else {
        return None;
    };
    match &stmt.expression {
        Expression::AssignmentExpression(assign) => {
            if assign.operator != AssignmentOperator::Assign {
                return None;
            }
            let target = assign.left.as_simple_assignment_target()?;
            let root = match target {
                SimpleAssignmentTarget::StaticMemberExpression(m) => chain_root(&m.object)?,
                SimpleAssignmentTarget::PrivateFieldExpression(m) => chain_root(&m.object)?,
                SimpleAssignmentTarget::ComputedMemberExpression(m) => {
                    if !matches!(
                        m.expression,
                        Expression::StringLiteral(_) | Expression::NumericLiteral(_)
                    ) {
                        return None;
                    }
                    chain_root(&m.object)?
                }
                _ => return None,
            };
            Some(Write {
                slot: Slot::Property(target),
                root,
                values: &[],
                value: Some(&assign.right),
                span: assign.span,
                target_span: target.span(),
            })
        }
        Expression::CallExpression(call) => {
            if call.optional || call.arguments.len() != 2 {
                return None;
            }
            let Expression::StaticMemberExpression(callee) = &call.callee else {
                return None;
            };
            if callee.optional || callee.property.name != "set" {
                return None;
            }
            let key = call.arguments.first()?.as_expression()?;
            if !matches!(
                helpers::unwrap_expr(key),
                Expression::StringLiteral(_) | Expression::NumericLiteral(_)
            ) {
                return None;
            }
            let root = chain_root(&callee.object)?;
            Some(Write {
                slot: Slot::Entry {
                    receiver: &callee.object,
                    key,
                },
                root,
                values: &call.arguments,
                value: None,
                span: call.span,
                target_span: callee.object.span(),
            })
        }
        _ => None,
    }
}

/// Root of a side-effect-free member chain: `a`, `this`, `a.b.c`, `this.#x.y`.
fn chain_root<'a>(expr: &'a Expression<'a>) -> Option<Root<'a>> {
    match expr {
        Expression::Identifier(id) => Some(Root::Ident(id.name.as_str())),
        Expression::ThisExpression(_) => Some(Root::This),
        Expression::StaticMemberExpression(m) if !m.optional => chain_root(&m.object),
        Expression::PrivateFieldExpression(m) if !m.optional => chain_root(&m.object),
        _ => None,
    }
}

/// Is the expression free of anything that could run code or read `root`: no calls, `new`,
/// `await`, `yield`, assignments, spreads, or mentions of `root`? Function expressions are fine:
/// they are created, not run.
fn is_simple(expr: &Expression<'_>, root: &Root<'_>) -> bool {
    match expr {
        Expression::StringLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BigIntLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::RegExpLiteral(_)
        | Expression::ArrowFunctionExpression(_)
        | Expression::FunctionExpression(_) => true,
        Expression::Identifier(id) => !matches!(root, Root::Ident(name) if *name == id.name),
        Expression::ThisExpression(_) => !matches!(root, Root::This),
        Expression::TemplateLiteral(t) => t.expressions.iter().all(|e| is_simple(e, root)),
        Expression::UnaryExpression(u) => {
            u.operator != UnaryOperator::Delete && is_simple(&u.argument, root)
        }
        Expression::BinaryExpression(b) => is_simple(&b.left, root) && is_simple(&b.right, root),
        Expression::LogicalExpression(l) => is_simple(&l.left, root) && is_simple(&l.right, root),
        Expression::ConditionalExpression(c) => {
            is_simple(&c.test, root)
                && is_simple(&c.consequent, root)
                && is_simple(&c.alternate, root)
        }
        Expression::StaticMemberExpression(m) => is_simple(&m.object, root),
        Expression::PrivateFieldExpression(m) => is_simple(&m.object, root),
        Expression::ComputedMemberExpression(m) => {
            is_simple(&m.object, root) && is_simple(&m.expression, root)
        }
        Expression::ArrayExpression(a) => a.elements.iter().all(|e| match e {
            ArrayExpressionElement::SpreadElement(_) => false,
            ArrayExpressionElement::Elision(_) => true,
            _ => e.as_expression().is_some_and(|e| is_simple(e, root)),
        }),
        Expression::ObjectExpression(o) => o.properties.iter().all(|p| match p {
            ObjectPropertyKind::SpreadProperty(_) => false,
            ObjectPropertyKind::ObjectProperty(p) => {
                is_simple(&p.value, root)
                    && (!p.computed || p.key.as_expression().is_some_and(|k| is_simple(k, root)))
            }
        }),
        Expression::TSAsExpression(e) => is_simple(&e.expression, root),
        Expression::TSSatisfiesExpression(e) => is_simple(&e.expression, root),
        Expression::TSNonNullExpression(e) => is_simple(&e.expression, root),
        Expression::TSTypeAssertion(e) => is_simple(&e.expression, root),
        _ => false,
    }
}

/// Does the class around `node` declare a setter or an auto-accessor (which may react to each
/// write)? Also true when `this` is not inside a class body we can see.
fn class_has_setters(ctx: &Ctx<'_>, node: &AstNode<'_>) -> bool {
    let Some(class) = ctx.ancestors(node).find_map(|n| match n.kind() {
        AstKind::Class(c) => Some(c),
        _ => None,
    }) else {
        return false;
    };
    class.body.body.iter().any(|element| match element {
        ClassElement::MethodDefinition(m) => m.kind == MethodDefinitionKind::Set,
        ClassElement::AccessorProperty(_) => true,
        _ => false,
    })
}

fn report(ctx: &mut Ctx<'_>, first: &Write<'_>, second: &Write<'_>) {
    let line = ctx.line_of(second.span.start);
    let target = util::preview(ctx.text(first.target_span), 40);
    let message = match &first.slot {
        Slot::Property(_) => {
            format!("Remove or retarget this assignment: '{target}' is assigned again on line {line} before it is read")
        }
        Slot::Entry { key, .. } => {
            let key = util::preview(ctx.text(key.span()), 30);
            format!("Remove or retarget this call: key {key} of '{target}' is set again on line {line} before it is read")
        }
    };
    let span = util::first_line(ctx, first.span);
    let again = util::first_line(ctx, second.span);
    ctx.emit(span, message).secondary(again, "Overwritten here");
}

/// A literal constant: string, number, boolean, `null`, `undefined`, `void 0`, `-1`, or a template
/// literal without substitutions.
fn is_constant(expr: &Expression<'_>) -> bool {
    match helpers::unwrap_expr(expr) {
        Expression::StringLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BigIntLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_) => true,
        Expression::TemplateLiteral(t) => t.expressions.is_empty(),
        Expression::Identifier(id) => id.name == "undefined",
        Expression::UnaryExpression(u) => {
            matches!(
                u.operator,
                UnaryOperator::Void | UnaryOperator::UnaryNegation | UnaryOperator::LogicalNot
            ) && is_constant(&u.argument)
        }
        _ => false,
    }
}

/// Is the code at `offset` directly preceded by a comment (only whitespace in between)?
fn comment_just_before(ctx: &Ctx<'_>, offset: u32) -> bool {
    let comments = ctx.comments();
    let i = comments.partition_point(|c| c.span.end <= offset);
    i > 0
        && comments.get(i - 1).is_some_and(|c| {
            ctx.text(Span::new(c.span.end, offset))
                .bytes()
                .all(|b| b.is_ascii_whitespace())
        })
}

/// Is there a comment on its own line between `from` and `to` (trailing comments on the line of
/// `from` do not count)?
fn own_line_comment_between(ctx: &Ctx<'_>, from: u32, to: u32) -> bool {
    let line = ctx.line_of(from);
    let comments = ctx.comments();
    let i = comments.partition_point(|c| c.span.start < from);
    comments[i..]
        .iter()
        .take_while(|c| c.span.end <= to)
        .any(|c| ctx.line_of(c.span.start) > line)
}
