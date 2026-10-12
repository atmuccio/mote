//! `js/no-loop-func`

use super::util;
use crate::prelude::*;

pub struct NoLoopFunc;

pub static NO_LOOP_FUNC: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["closure", "async"],
    remediation_minutes: 10,
    description: r#"Flags a function created inside a loop and scheduled to run later — a timer, a promise
callback, an event handler, a function stored in an array or a property — that reads a loop
variable shared by all iterations: a `var` declared in the loop, or an outer variable that the
loop keeps updating.

**Why:** a `var` (or a variable declared outside the loop) is one binding for the whole loop, not
one per iteration. Functions that run after the loop has moved on all see its *final* value: every
handler reports the last index, every timer processes the last item.

**Fix:** declare the loop variable with `let` or `const` in the loop head or body (each iteration
then gets its own binding), or pass the current value to the callback explicitly.

```js
for (var i = 0; i < buttons.length; i++) {
  buttons[i].addEventListener('click', () => select(i)); // flagged: every button selects the last index
}

for (let i = 0; i < buttons.length; i++) {
  buttons[i].addEventListener('click', () => select(i)); // compliant: one `i` per iteration
}
```

Only functions whose call is visibly deferred are checked: passed to `setTimeout`, `setInterval`,
`setImmediate`, `requestAnimationFrame`, `requestIdleCallback`, `queueMicrotask`, or
`process.nextTick`, to `.then` / `.catch` / `.finally`, to `.addEventListener` / `.on` / `.once` /
`.one` / `.subscribe`, to `.push` / `.unshift`, assigned to a property (`el.onclick = …`), or given
as an `on…` JSX prop. Callbacks that run at once (`forEach`, `map`, `new Promise(…)`) are not
reported, and neither are functions whose scheduling is awaited, yielded, or returned, or that are
followed by a statement ending the loop (`break`, `return`, `throw`). Loop variables declared with
`let` or `const` are safe.
"#,
    ..RuleMeta::new(
        "js/no-loop-func",
        "js",
        "Functions scheduled from a loop should not capture a variable shared by all iterations",
    )
};

const TIMERS: &[&str] = &[
    "setTimeout",
    "setInterval",
    "setImmediate",
    "requestAnimationFrame",
    "requestIdleCallback",
    "queueMicrotask",
];

const DEFERRING_METHODS: &[&str] = &[
    "then",
    "catch",
    "finally",
    "addEventListener",
    "on",
    "once",
    "one",
    "subscribe",
    "push",
    "unshift",
];

impl JsRule for NoLoopFunc {
    fn meta(&self) -> &'static RuleMeta {
        &NO_LOOP_FUNC
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::ArrowFunctionExpression, AstType::Function]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        match node.kind() {
            AstKind::ArrowFunctionExpression(_) => {}
            AstKind::Function(f) if f.is_expression() => {}
            _ => return,
        }
        if !is_deferred(ctx, node) {
            return;
        }
        let Some(loops) = enclosing_loops(ctx, node) else {
            return;
        };
        if loops.is_empty() || iteration_ends_loop(ctx, node) {
            return;
        }
        let span = node.span();
        let mut shared: Vec<SymbolId> = Vec::new();
        let mut first: Option<Span> = None;
        for inner in util::descendants(ctx, node) {
            let AstKind::IdentifierReference(id) = inner.kind() else {
                continue;
            };
            let Some(symbol) = ctx.reference_symbol(id) else {
                continue;
            };
            if shared.contains(&symbol) || util::within(ctx.symbol_span(symbol), span) {
                continue;
            }
            if !loops
                .iter()
                .any(|&l| is_shared_loop_variable(ctx, symbol, l))
            {
                continue;
            }
            shared.push(symbol);
            first.get_or_insert(id.span);
        }
        let Some(first) = first else {
            return;
        };
        let names = shared
            .iter()
            .map(|&s| format!("'{}'", ctx.symbol_name(s)))
            .collect::<Vec<_>>()
            .join(", ");
        let verb = if shared.len() == 1 { "is" } else { "are" };
        ctx.emit(
            first,
            format!(
                "{names} {verb} shared by every iteration of the loop: this function runs later and sees the final value; declare it with 'let' or pass the value in"
            ),
        );
    }
}

/// Is the function handed to something that calls it after the current statement: a timer, a
/// promise callback, an event subscription, a stored array entry, a property, or an `on…` prop?
fn is_deferred(ctx: &Ctx<'_>, function: &AstNode<'_>) -> bool {
    let span = function.span();
    let Some(parent) = ctx.outer_parent(function) else {
        return false;
    };
    match parent.kind() {
        AstKind::CallExpression(call) => {
            if !call.arguments.iter().any(|a| {
                a.as_expression()
                    .is_some_and(|e| helpers::unwrap_expr(e).span() == span)
            }) {
                return false;
            }
            match helpers::unwrap_expr(&call.callee) {
                Expression::Identifier(id) => {
                    TIMERS.contains(&id.name.as_str()) && ctx.is_global_reference(id)
                }
                Expression::StaticMemberExpression(m) => {
                    let method = m.property.name.as_str();
                    if TIMERS.contains(&method) {
                        return matches!(helpers::unwrap_expr(&m.object), Expression::Identifier(o)
                            if matches!(o.name.as_str(), "window" | "globalThis" | "self" | "global"));
                    }
                    if method == "nextTick" {
                        return ctx.is_global(&m.object, "process");
                    }
                    DEFERRING_METHODS.contains(&method)
                }
                _ => false,
            }
        }
        AstKind::AssignmentExpression(assign) => {
            assign.operator == AssignmentOperator::Assign
                && helpers::unwrap_expr(&assign.right).span() == span
                && assign.left.as_member_expression().is_some()
        }
        AstKind::JSXExpressionContainer(_) => matches!(
            ctx.parent_kind(parent),
            Some(AstKind::JSXAttribute(attr))
                if matches!(&attr.name, JSXAttributeName::Identifier(n)
                    if n.name.starts_with("on") && n.name.len() > 2)
        ),
        _ => false,
    }
}

/// The loops (innermost first) that run the function's creation repeatedly, within the same
/// function. `None` when the creation is awaited, yielded, or returned: then the loop does not go
/// on before the function has run (or the loop ends).
/// Ancestors [`enclosing_loops`] walks before giving up (`None`: not reported). Real code puts a
/// handful of expressions between a callback and its enclosing function; a chain of thousands
/// of `.then(…)` calls made the walk from every callback quadratic.
const MAX_LOOP_WALK: usize = 512;

fn enclosing_loops(ctx: &Ctx<'_>, function: &AstNode<'_>) -> Option<Vec<NodeId>> {
    let mut loops = Vec::new();
    let mut child = function.span();
    for (steps, ancestor) in ctx.ancestors(function).enumerate() {
        if steps >= MAX_LOOP_WALK {
            return None;
        }
        let kind = ancestor.kind();
        match kind {
            AstKind::AwaitExpression(_)
            | AstKind::YieldExpression(_)
            | AstKind::ReturnStatement(_) => {
                if loops.is_empty() {
                    return None;
                }
            }
            AstKind::ForStatement(s) => {
                if s.body.span() == child
                    || s.test.as_ref().is_some_and(|t| t.span() == child)
                    || s.update.as_ref().is_some_and(|u| u.span() == child)
                {
                    loops.push(ancestor.id());
                }
            }
            AstKind::ForInStatement(s) if s.body.span() == child => loops.push(ancestor.id()),
            AstKind::ForOfStatement(s) if s.body.span() == child => loops.push(ancestor.id()),
            AstKind::WhileStatement(_) | AstKind::DoWhileStatement(_) => {
                loops.push(ancestor.id());
            }
            _ => {}
        }
        if matches!(
            kind,
            AstKind::Function(_)
                | AstKind::ArrowFunctionExpression(_)
                | AstKind::Class(_)
                | AstKind::StaticBlock(_)
        ) {
            break;
        }
        child = kind.span();
    }
    Some(loops)
}

/// Is the statement that creates the function followed, in its block, by a statement that always
/// leaves the loop (`break`, `return`, `throw`)? Then the shared variable keeps the value it has now.
fn iteration_ends_loop(ctx: &Ctx<'_>, function: &AstNode<'_>) -> bool {
    let mut child = function.span();
    for ancestor in ctx.ancestors(function) {
        let list = match ancestor.kind() {
            AstKind::BlockStatement(b) => &b.body,
            AstKind::SwitchCase(c) => &c.consequent,
            AstKind::FunctionBody(_) => return false,
            kind if kind.is_iteration_statement() => return false,
            kind => {
                child = kind.span();
                continue;
            }
        };
        let index = list.partition_point(|s| s.span().start < child.start);
        return list.get(index + 1..).unwrap_or(&[]).iter().any(leaves_loop);
    }
    false
}

fn leaves_loop(stmt: &Statement<'_>) -> bool {
    match stmt {
        Statement::ReturnStatement(_)
        | Statement::ThrowStatement(_)
        | Statement::BreakStatement(_) => true,
        Statement::BlockStatement(b) => b.body.iter().any(leaves_loop),
        _ => false,
    }
}

/// Is `symbol` one binding shared by all iterations of `loop_id`: a `var` declared in the loop
/// (head or body), or a variable declared before the loop that the loop's own head or test
/// updates or tests while its body or update writes it?
fn is_shared_loop_variable(ctx: &Ctx<'_>, symbol: SymbolId, loop_id: NodeId) -> bool {
    let loop_node = ctx.node(loop_id);
    let loop_span = loop_node.span();
    let declared = ctx.symbol_span(symbol);
    let decl = ctx.symbol_declaration(symbol);
    let AstKind::VariableDeclarator(d) = decl.kind() else {
        return false;
    };
    if util::within(declared, loop_span) {
        return d.kind == VariableDeclarationKind::Var;
    }
    if d.kind == VariableDeclarationKind::Const || declared.start >= loop_span.start {
        return false;
    }
    // Declared before the loop: it must be the loop's counter or cursor.
    let (test, update) = match loop_node.kind() {
        AstKind::ForStatement(s) => (
            s.test.as_ref().map(GetSpan::span),
            s.update.as_ref().map(GetSpan::span),
        ),
        AstKind::WhileStatement(s) => (Some(s.test.span()), None),
        AstKind::DoWhileStatement(s) => (Some(s.test.span()), None),
        _ => (None, None),
    };
    let Some(test) = test else {
        return false;
    };
    let mut tested = false;
    let mut written = false;
    for reference in ctx.references_of(symbol) {
        let at = ctx.node(reference.node_id()).span();
        if util::within(at, test) {
            tested = true;
        }
        if reference.is_write()
            && (util::within(at, loop_span) || update.is_some_and(|u| util::within(at, u)))
        {
            written = true;
        }
    }
    tested && written
}
