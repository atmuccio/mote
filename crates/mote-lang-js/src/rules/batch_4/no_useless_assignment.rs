//! `js/no-useless-assignment` — dead stores to function-local variables, found with a conservative
//! straight-line scan (no control-flow graph).

use super::util;
use crate::prelude::*;

pub struct NoUselessAssignment;

pub static NO_USELESS_ASSIGNMENT: RuleMeta = RuleMeta {
    severity: Severity::Medium,
    tags: &["unused", "suspicious"],
    remediation_minutes: 5,
    description: r#"Flags values stored in a local variable that can never be read: the variable is
assigned again before anything reads it, or the function ends (or returns) first.

**Why:** a dead store is wasted work at best. More often it is a mistake: the wrong variable was
assigned, a read was forgotten, or an initial value is misleading because every path replaces it.

**Fix:** remove the useless assignment (or initializer), or use the value where it was meant to be
used.

```js
function label(user) {
  let text = 'unknown';        // flagged: overwritten on the next line
  text = user.name;
  if (!text) {
    text = 'anonymous';
    return 'nobody';           // the value 'anonymous' is never read
  }
  return text;
}

function label(user) {
  let text = user.name;        // compliant
  if (!text) text = 'anonymous';
  return text;
}
```

The analysis is deliberately conservative. Only local variables and parameters of the enclosing
function are considered, and a store is reported only when the code between it and the
overwrite (or the exit) runs straight through: no `break`/`continue`, no enclosing loop between the
store and the end of the function when the variable could be read on the next iteration, and no
enclosing `try`. An `if`/`else` whose every branch assigns the variable (or returns) before
reading it counts as an overwrite, and so does a `switch` with a `default` clause in which every
clause (following fall-through) assigns it or returns before a `break`, and a plain nested block
`{ … }` that assigns it before reading it. Increment and compound-assignment statements
(`count++;`, `total += x;`) are stores too: their new value is flagged when nothing reads it. A
`var` declared several times in the same function is one variable, and a repeated
`var x = value;` overwrites it like an assignment. Variables used by nested functions or classes
are never reported, and neither are variables that are never read at all, including variables
that are only ever incremented or updated from their own value (see `js/no-unused-vars`).

```js
function next(ids) {
  let index = 0;
  while (ids[index]) index++;
  register(ids[index]);
  index++;                     // flagged: the new value is never read
}
```
"#,
    ..RuleMeta::new(
        "js/no-useless-assignment",
        "js",
        "Values assigned to variables should be read",
    )
};

/// Maximum number of sibling statements scanned after one store.
const SCAN_LIMIT: usize = 400;

#[derive(Clone, Copy)]
enum StoreKind {
    /// `x = value;` — the node is the `ExpressionStatement`.
    Assign,
    /// `let x = value;` — the node is the `VariableDeclarator`.
    Init,
    /// `x++;` or `x += value;` — the node is the `ExpressionStatement`.
    Update,
}

#[derive(Default)]
struct State {
    stores: Vec<(NodeId, SymbolId, StoreKind)>,
    /// Start offsets of every `break` / `continue` statement, in source order.
    jumps: Vec<u32>,
}

/// Facts about a candidate variable, computed once.
struct SymbolInfo {
    eligible: bool,
    /// Start offsets of every reference, sorted.
    positions: Vec<u32>,
    /// The function that declares the variable.
    unit: Option<NodeId>,
}

enum Verdict {
    Overwritten(Span),
    NeverRead,
    Unknown,
}

impl JsRule for NoUselessAssignment {
    fn meta(&self) -> &'static RuleMeta {
        &NO_USELESS_ASSIGNMENT
    }

    fn types(&self) -> &'static [AstType] {
        &[
            AstType::ExpressionStatement,
            AstType::VariableDeclaration,
            AstType::BreakStatement,
            AstType::ContinueStatement,
        ]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        // `with` and direct `eval` can read any local by name.
        !ctx.file_contains(AstType::WithStatement) && !util::uses_global(ctx, "eval")
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        match node.kind() {
            AstKind::BreakStatement(s) => ctx.state::<State>().jumps.push(s.span.start),
            AstKind::ContinueStatement(s) => ctx.state::<State>().jumps.push(s.span.start),
            AstKind::ExpressionStatement(stmt) => {
                let (id, kind) = match &stmt.expression {
                    Expression::AssignmentExpression(assign) => {
                        let AssignmentTarget::AssignmentTargetIdentifier(id) = &assign.left else {
                            return;
                        };
                        if assign.operator == AssignmentOperator::Assign {
                            (id, StoreKind::Assign)
                        } else if assign.operator.is_logical() {
                            // `x ||= v` stores only sometimes.
                            return;
                        } else {
                            (id, StoreKind::Update)
                        }
                    }
                    Expression::UpdateExpression(update) => {
                        let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) =
                            &update.argument
                        else {
                            return;
                        };
                        (id, StoreKind::Update)
                    }
                    _ => return,
                };
                // `() => x++`: the update's value is the function's result.
                if matches!(kind, StoreKind::Update) && is_arrow_expression_body(ctx, node) {
                    return;
                }
                let Some(symbol) = ctx.reference_symbol(id) else {
                    return;
                };
                ctx.state::<State>().stores.push((node.id(), symbol, kind));
            }
            AstKind::VariableDeclaration(decl) => {
                if !matches!(
                    decl.kind,
                    VariableDeclarationKind::Let | VariableDeclarationKind::Var
                ) || decl.declare
                    || !is_statement_list(ctx.parent_kind(node))
                {
                    return;
                }
                for declarator in &decl.declarations {
                    if declarator.init.is_none() {
                        continue;
                    }
                    let BindingPattern::BindingIdentifier(id) = &declarator.id else {
                        continue;
                    };
                    let Some(symbol) = ctx.binding_symbol(id) else {
                        continue;
                    };
                    let id = declarator.node_id.get();
                    ctx.state::<State>()
                        .stores
                        .push((id, symbol, StoreKind::Init));
                }
            }
            _ => {}
        }
    }

    fn run_once<'a>(&self, ctx: &mut Ctx<'a>) {
        let state = std::mem::take(ctx.state::<State>());
        if state.stores.is_empty() {
            return;
        }
        let mut infos: FxHashMap<SymbolId, SymbolInfo> = FxHashMap::default();
        let mut reports: Vec<(Span, String, Option<Span>)> = Vec::new();
        for &(node_id, symbol, kind) in &state.stores {
            let info = infos
                .entry(symbol)
                .or_insert_with(|| symbol_info(ctx, symbol));
            if !info.eligible {
                continue;
            }
            let node = ctx.node(node_id);
            // The store must run directly in the declaring function, outside any `try`.
            if store_unit(ctx, node) != info.unit {
                continue;
            }
            let (statement, report_span) = match (kind, node.kind()) {
                (StoreKind::Assign | StoreKind::Update, AstKind::ExpressionStatement(stmt)) => {
                    let span = stmt.expression.span();
                    (node, span)
                }
                (StoreKind::Init, AstKind::VariableDeclarator(d)) => {
                    let Some(statement) = ctx.parent(node) else {
                        continue;
                    };
                    // The rest of the declaration (`let a = 1, b = a`) runs after the store.
                    let rest = Span::new(d.span.end, statement.span().end);
                    if has_position_in(&info.positions, rest) {
                        continue;
                    }
                    (statement, d.span)
                }
                _ => continue,
            };
            let verdict = scan(ctx, statement, symbol, &info.positions, &state.jumps);
            let name = ctx.symbol_name(symbol);
            let report_span = util::first_line(ctx, report_span);
            match (verdict, kind) {
                (Verdict::Overwritten(by), StoreKind::Assign) => reports.push((
                    report_span,
                    format!(
                        "Remove this useless assignment to '{name}': the value is overwritten before it is read"
                    ),
                    Some(by),
                )),
                (Verdict::Overwritten(by), StoreKind::Init) => reports.push((
                    report_span,
                    format!(
                        "Remove the useless initial value of '{name}': it is overwritten before it is read"
                    ),
                    Some(by),
                )),
                (Verdict::NeverRead, StoreKind::Assign) => reports.push((
                    report_span,
                    format!("Remove this useless assignment to '{name}': the value is never read"),
                    None,
                )),
                (Verdict::NeverRead, StoreKind::Init) => reports.push((
                    report_span,
                    format!("Remove the useless initial value of '{name}': it is never read"),
                    None,
                )),
                (Verdict::Overwritten(by), StoreKind::Update) => reports.push((
                    report_span,
                    format!(
                        "Remove this useless update of '{name}': the new value is overwritten before it is read"
                    ),
                    Some(by),
                )),
                (Verdict::NeverRead, StoreKind::Update) => reports.push((
                    report_span,
                    format!("Remove this useless update of '{name}': the new value is never read"),
                    None,
                )),
                (Verdict::Unknown, _) => {}
            }
        }
        for (span, message, by) in reports {
            let emitted = ctx.emit(span, message);
            if let Some(by) = by {
                emitted.secondary(by, "Overwritten here");
            }
        }
    }
}

/// Containers whose child statements form a list we can scan.
fn is_statement_list(kind: Option<AstKind<'_>>) -> bool {
    matches!(
        kind,
        Some(
            AstKind::FunctionBody(_)
                | AstKind::BlockStatement(_)
                | AstKind::SwitchCase(_)
                | AstKind::StaticBlock(_)
                | AstKind::Program(_)
        )
    )
}

/// The execution unit of a scope: the nearest function, arrow function, static block, or class
/// scope (class bodies run field initializers at other times). `None` at the top level.
fn unit_of(ctx: &Ctx<'_>, scope: ScopeId) -> Option<ScopeId> {
    let scoping = ctx.scoping();
    for s in scoping.scope_ancestors(scope) {
        let flags = scoping.scope_flags(s);
        if flags.intersects(ScopeFlags::Function | ScopeFlags::Arrow | ScopeFlags::ClassStaticBlock)
            || matches!(ctx.node(scoping.get_node_id(s)).kind(), AstKind::Class(_))
        {
            return Some(s);
        }
        if flags.contains(ScopeFlags::Top) {
            return None;
        }
    }
    None
}

/// The unit of a store, or `None` when it sits inside a `try` statement of that unit.
fn store_unit(ctx: &Ctx<'_>, node: &AstNode<'_>) -> Option<NodeId> {
    for ancestor in ctx.ancestors(node) {
        match ancestor.kind() {
            AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) | AstKind::Class(_) => {
                return Some(ancestor.id());
            }
            AstKind::TryStatement(_) | AstKind::CatchClause(_) | AstKind::WithStatement(_) => {
                return None;
            }
            _ => {}
        }
    }
    None
}

fn symbol_info(ctx: &Ctx<'_>, symbol: SymbolId) -> SymbolInfo {
    let ineligible = SymbolInfo {
        eligible: false,
        positions: Vec::new(),
        unit: None,
    };
    let flags = ctx.symbol_flags(symbol);
    if !flags.intersects(SymbolFlags::FunctionScopedVariable | SymbolFlags::BlockScopedVariable)
        || flags.intersects(
            SymbolFlags::ConstVariable
                | SymbolFlags::CatchVariable
                | SymbolFlags::Ambient
                | SymbolFlags::Function
                | SymbolFlags::Class
                | SymbolFlags::Import,
        )
    {
        return ineligible;
    }
    // A `var` declared several times in one function is still one variable; any other
    // redeclaration (a parameter, a function declaration) is left alone.
    let redeclarations = ctx.scoping().symbol_redeclarations(symbol);
    if !redeclarations
        .iter()
        .all(|r| is_var_declarator(ctx.node(r.declaration).kind()))
    {
        return ineligible;
    }
    let scoping = ctx.scoping();
    let Some(unit_scope) = unit_of(ctx, scoping.symbol_scope_id(symbol)) else {
        return ineligible;
    };
    let unit_id = scoping.get_node_id(unit_scope);
    let unit_node = ctx.node(unit_id);
    if !matches!(
        unit_node.kind(),
        AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
    ) {
        return ineligible;
    }
    match ctx.symbol_declaration(symbol).kind() {
        AstKind::VariableDeclarator(d) => {
            if !matches!(
                d.kind,
                VariableDeclarationKind::Let | VariableDeclarationKind::Var
            ) {
                return ineligible;
            }
        }
        AstKind::FormalParameter(_) => {
            // In sloppy-mode functions, `arguments[i]` aliases the parameters.
            if util::uses_arguments_in(ctx, unit_node.span()) {
                return ineligible;
            }
        }
        _ => return ineligible,
    }
    let mut positions = Vec::new();
    let mut read = false;
    for reference in ctx.references_of(symbol) {
        // Captured by a nested function or class: its reads cannot be placed in the scan.
        if unit_of(ctx, reference.scope_id()) != Some(unit_scope) {
            return ineligible;
        }
        read |= reference.flags() != ReferenceFlags::Write
            && !is_discarded_self_update(ctx, reference.node_id());
        positions.push(ctx.node(reference.node_id()).span().start);
    }
    // Repeated `var` declarations mention the variable too (`var x = 2` overwrites it).
    for redeclaration in redeclarations {
        positions.push(redeclaration.span.start);
    }
    // Variables that are never read at all (only written, or only updated from their own value
    // as in `count++;`) are left to `js/no-unused-vars`.
    if !read {
        return ineligible;
    }
    positions.sort_unstable();
    SymbolInfo {
        eligible: true,
        positions,
        unit: Some(unit_id),
    }
}

/// Is any offset of the sorted `positions` inside `span`?
fn has_position_in(positions: &[u32], span: Span) -> bool {
    let i = positions.partition_point(|&p| p < span.start);
    positions.get(i).is_some_and(|&p| p < span.end)
}

/// Follow the statements that run after `statement` until the variable is read, overwritten, or the
/// function ends.
fn scan<'a>(
    ctx: &Ctx<'a>,
    statement: &'a AstNode<'a>,
    symbol: SymbolId,
    positions: &[u32],
    jumps: &[u32],
) -> Verdict {
    let mut current = statement;
    let mut scanned = 0usize;
    'climb: loop {
        let Some(parent) = ctx.parent(current) else {
            return Verdict::Unknown;
        };
        let list: Option<&'a [Statement<'a>]> = match parent.kind() {
            AstKind::FunctionBody(b) => Some(&b.statements),
            AstKind::BlockStatement(b) => Some(&b.body),
            AstKind::SwitchCase(c) => Some(&c.consequent),
            AstKind::StaticBlock(b) => Some(&b.body),
            AstKind::IfStatement(_) => None,
            _ => return Verdict::Unknown,
        };
        if let Some(list) = list {
            let start = current.span().start;
            let index = list.partition_point(|s| s.span().start < start);
            if list.get(index).map(GetSpan::span) != Some(current.span()) {
                return Verdict::Unknown;
            }
            for sibling in list.get(index + 1..).unwrap_or(&[]) {
                scanned += 1;
                if scanned > SCAN_LIMIT {
                    return Verdict::Unknown;
                }
                let span = sibling.span();
                if has_position_in(positions, span) {
                    if let Some(by) = overwrite_span(ctx, sibling, symbol, positions) {
                        return Verdict::Overwritten(by);
                    }
                    if if_kills(ctx, sibling, symbol, positions, jumps)
                        || switch_kills(ctx, sibling, symbol, positions, jumps)
                        || block_kills(ctx, sibling, symbol, positions, jumps)
                    {
                        return Verdict::Overwritten(util::first_line(ctx, span));
                    }
                    return Verdict::Unknown;
                }
                // A plain `break;` of a `switch` continues after the `switch`.
                if let Statement::BreakStatement(b) = sibling {
                    if b.label.is_none() {
                        if let Some(switch) = enclosing_switch(ctx, parent) {
                            current = switch;
                            continue 'climb;
                        }
                    }
                    return Verdict::Unknown;
                }
                if has_position_in(jumps, span) {
                    return Verdict::Unknown;
                }
                if util::always_exits(sibling) {
                    return Verdict::NeverRead;
                }
            }
        }
        match parent.kind() {
            // The end of the declaring function: nothing reads the value any more.
            AstKind::FunctionBody(_) => return Verdict::NeverRead,
            AstKind::BlockStatement(_) | AstKind::IfStatement(_) => current = parent,
            _ => return Verdict::Unknown,
        }
    }
}

/// The `switch` statement an unlabeled `break` in `node` leaves, when no loop is closer.
fn enclosing_switch<'a>(ctx: &Ctx<'a>, node: &'a AstNode<'a>) -> Option<&'a AstNode<'a>> {
    std::iter::once(node)
        .chain(ctx.ancestors(node))
        .find(|n| {
            let kind = n.kind();
            matches!(kind, AstKind::SwitchStatement(_))
                || kind.is_iteration_statement()
                || kind.is_function_like()
        })
        .filter(|n| matches!(n.kind(), AstKind::SwitchStatement(_)))
}

/// `if (test) { … x = a; } else { … x = b; }`: an `if`/`else` whose test does not mention the
/// variable and whose every branch overwrites it (or leaves the function) before reading it.
fn if_kills(
    ctx: &Ctx<'_>,
    statement: &Statement<'_>,
    symbol: SymbolId,
    positions: &[u32],
    jumps: &[u32],
) -> bool {
    let Statement::IfStatement(stmt) = statement else {
        return false;
    };
    let Some(alternate) = &stmt.alternate else {
        return false;
    };
    !has_position_in(positions, stmt.test.span())
        && branch_kills(ctx, &stmt.consequent, symbol, positions, jumps)
        && branch_kills(ctx, alternate, symbol, positions, jumps)
}

/// `switch (k) { case a: x = 1; break; … default: x = 0; }`: a `switch` with a `default` clause
/// whose discriminant and case labels do not mention the variable, and where every run — entering
/// at any clause and falling through until a `break` — overwrites the variable (or leaves the
/// function) before reading it.
fn switch_kills(
    ctx: &Ctx<'_>,
    statement: &Statement<'_>,
    symbol: SymbolId,
    positions: &[u32],
    jumps: &[u32],
) -> bool {
    let Statement::SwitchStatement(stmt) = statement else {
        return false;
    };
    if has_position_in(positions, stmt.discriminant.span())
        || !stmt.cases.iter().any(|c| c.test.is_none())
        || stmt.cases.iter().any(|c| {
            c.test
                .as_ref()
                .is_some_and(|t| has_position_in(positions, t.span()))
        })
    {
        return false;
    }
    (0..stmt.cases.len()).all(|entry| {
        for case in stmt.cases.get(entry..).unwrap_or(&[]) {
            for statement in &case.consequent {
                let span = statement.span();
                if has_position_in(positions, span) {
                    return overwrite_span(ctx, statement, symbol, positions).is_some()
                        || if_kills(ctx, statement, symbol, positions, jumps)
                        || switch_kills(ctx, statement, symbol, positions, jumps)
                        || block_kills(ctx, statement, symbol, positions, jumps);
                }
                // A plain `break` leaves the switch with the old value still live.
                if matches!(statement, Statement::BreakStatement(b) if b.label.is_none()) {
                    return false;
                }
                if has_position_in(jumps, span) {
                    return false;
                }
                if util::always_exits(statement) {
                    return true;
                }
            }
            // No exit: fall through into the next clause.
        }
        // Falling off the end of the switch keeps the old value.
        false
    })
}

/// `{ … x = value; … }`: a plain nested block that overwrites the variable (or leaves the function)
/// before reading it.
fn block_kills(
    ctx: &Ctx<'_>,
    statement: &Statement<'_>,
    symbol: SymbolId,
    positions: &[u32],
    jumps: &[u32],
) -> bool {
    matches!(statement, Statement::BlockStatement(_))
        && branch_kills(ctx, statement, symbol, positions, jumps)
}

/// Does every run through `branch` overwrite the variable, or leave the function, before reading it?
fn branch_kills(
    ctx: &Ctx<'_>,
    branch: &Statement<'_>,
    symbol: SymbolId,
    positions: &[u32],
    jumps: &[u32],
) -> bool {
    let list = match branch {
        Statement::BlockStatement(b) => &b.body[..],
        other => std::slice::from_ref(other),
    };
    for statement in list {
        let span = statement.span();
        if has_position_in(positions, span) {
            return overwrite_span(ctx, statement, symbol, positions).is_some()
                || if_kills(ctx, statement, symbol, positions, jumps)
                || switch_kills(ctx, statement, symbol, positions, jumps)
                || block_kills(ctx, statement, symbol, positions, jumps);
        }
        if has_position_in(jumps, span) {
            return false;
        }
        if util::always_exits(statement) {
            return true;
        }
    }
    false
}

/// `x = value;` (or a repeated `var x = value;`) that overwrites the variable without reading it
/// first.
fn overwrite_span(
    ctx: &Ctx<'_>,
    statement: &Statement<'_>,
    symbol: SymbolId,
    positions: &[u32],
) -> Option<Span> {
    if let Statement::VariableDeclaration(decl) = statement {
        return redeclaration_overwrite(ctx, decl, symbol, positions);
    }
    let Statement::ExpressionStatement(stmt) = statement else {
        return None;
    };
    let Expression::AssignmentExpression(assign) = &stmt.expression else {
        return None;
    };
    if assign.operator != AssignmentOperator::Assign {
        return None;
    }
    let AssignmentTarget::AssignmentTargetIdentifier(id) = &assign.left else {
        return None;
    };
    if ctx.reference_symbol(id) != Some(symbol) || has_position_in(positions, assign.right.span()) {
        return None;
    }
    Some(util::first_line(ctx, assign.span))
}

/// `var x = value;` repeating the declaration of `x`: an overwrite when the declarator is the first
/// mention of the variable in the statement and its initializer does not read it.
fn redeclaration_overwrite(
    ctx: &Ctx<'_>,
    decl: &VariableDeclaration<'_>,
    symbol: SymbolId,
    positions: &[u32],
) -> Option<Span> {
    if decl.kind != VariableDeclarationKind::Var {
        return None;
    }
    let first = positions
        .get(positions.partition_point(|&p| p < decl.span.start))
        .copied()?;
    decl.declarations.iter().find_map(|d| {
        let BindingPattern::BindingIdentifier(id) = &d.id else {
            return None;
        };
        let init = d.init.as_ref()?;
        (ctx.binding_symbol(id) == Some(symbol)
            && id.span.start == first
            && !has_position_in(positions, init.span()))
        .then(|| util::first_line(ctx, d.span))
    })
}

/// A declarator of a `var` statement.
fn is_var_declarator(kind: AstKind<'_>) -> bool {
    matches!(kind, AstKind::VariableDeclarator(d) if d.kind == VariableDeclarationKind::Var)
}

/// An arrow function's expression body is represented as an expression statement.
fn is_arrow_expression_body(ctx: &Ctx<'_>, stmt: &AstNode<'_>) -> bool {
    let Some(body) = ctx.parent(stmt) else {
        return false;
    };
    matches!(body.kind(), AstKind::FunctionBody(_))
        && matches!(ctx.parent_kind(body), Some(AstKind::ArrowFunctionExpression(a)) if a.expression)
}

/// Is the reference the target of `x++` / `x += v` whose result is thrown away (a statement, a
/// `for` update, or a comma sequence in those places)? Such an update reads the variable only to
/// write it back.
fn is_discarded_self_update(ctx: &Ctx<'_>, reference: NodeId) -> bool {
    let Some(parent) = ctx.parent(reference) else {
        return false;
    };
    // The reference must be the updated target, not a value read on the right-hand side.
    let target = ctx.node(reference).span();
    match parent.kind() {
        AstKind::UpdateExpression(u) if u.argument.span() == target => {}
        AstKind::AssignmentExpression(a)
            if a.operator != AssignmentOperator::Assign
                && !a.operator.is_logical()
                && a.left.span() == target => {}
        _ => return false,
    }
    let mut current = parent;
    loop {
        let Some(up) = ctx.parent(current) else {
            return false;
        };
        match up.kind() {
            AstKind::SequenceExpression(_) => current = up,
            AstKind::ExpressionStatement(_) => return !is_arrow_expression_body(ctx, up),
            AstKind::ForStatement(f) => {
                return f
                    .update
                    .as_ref()
                    .is_some_and(|u| u.span() == current.span());
            }
            _ => return false,
        }
    }
}
