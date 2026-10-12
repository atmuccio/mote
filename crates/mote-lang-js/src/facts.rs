//! File facts computed in one linear pass over the flat node list, *before* any rule runs:
//! per-function metrics, class/statement counts, node-type presence, and the name ranges used to
//! attach a `Class#method` / `function` symbol to every finding.
//!
//! Oxc stores nodes in pre-order with parent links, so every per-node value is derived from the
//! parent's value in O(1): the owning *container* (function unit or class), the cognitive-complexity
//! nesting level, and the control-flow depth.
//!
//! ## Function units
//!
//! A *unit* gets its own [`FunctionMetric`]: function declarations, class members (methods,
//! accessors, constructors, arrow/function-valued properties), object-literal members, functions
//! assigned to a variable or property, `export default` functions, named function expressions, and
//! any function that has no enclosing unit (top-level callbacks and IIFEs). Every other anonymous
//! function or arrow — typically a callback passed inline — is folded into its enclosing unit: its
//! decision points count towards that unit and it adds one cognitive nesting level.
//!
//! ## Test suites
//!
//! The callback of a test-suite call (`describe`, `context`, `suite`, with `.only`, `.skip`,
//! `.each(…)` and the like; `fdescribe`, `xdescribe`, `xcontext`; Playwright's `test.describe`),
//! called as a test framework's API (a global or an import), is not a unit: it groups tests the
//! way a class groups methods, and becomes a container without a metric. The callbacks directly
//! in it — tests, hooks — are units of their own, measured like top-level test callbacks, and
//! nested suites are containers too. So function-level rules (length, complexity) look at each
//! test rather than at the whole suite. Names stay what the code was named while it was folded
//! into the suite's callback, so finding symbols (and fingerprints) do not change: a suite and
//! the tests in it carry the name of the suite's callback (`<anonymous>`, or the unit it is in).
//!
//! ## Cognitive complexity
//!
//! Structural increments (`+1 + nesting`): `if`, `? :`, `switch`, `for`, `for…in`, `for…of`,
//! `while`, `do…while`, `catch`. Flat increments (`+1`): `else if`, `else`, each run of the same
//! logical operator (`&&`, `||`, `??`), labelled `break`/`continue`. Nesting rises inside the
//! bodies of those structures (not their conditions) and inside folded callbacks.

use mote_core::{FunctionMetric, LineIndex};
use oxc_ast::ast::{
    AssignmentTarget, BindingPattern, Expression, Function, IdentifierReference, PropertyKey,
    SimpleAssignmentTarget, Statement,
};
use oxc_ast::ast_kind::AST_TYPE_MAX;
use oxc_ast::{AstKind, AstType};
use oxc_semantic::{AstNodes, NodeId, Semantic, SymbolFlags};
use oxc_span::{GetSpan, Span};

const NONE: u32 = u32::MAX;
const PRESENCE_WORDS: usize = (AST_TYPE_MAX as usize >> 6) + 1;

/// A function unit, class or test suite, used to name findings.
struct Container {
    name: String,
    start: u32,
    end: u32,
    /// Enclosing container by span containment (set after sorting).
    parent: u32,
    /// Index into `functions` for function units.
    metric: u32,
    /// A test suite's callback (`describe('…', () => { … })`): no metric of its own, and the
    /// callbacks in it are units.
    suite: bool,
}

pub(crate) struct Facts {
    pub functions: Vec<FunctionMetric>,
    pub classes: u32,
    pub statements: u32,
    presence: [u64; PRESENCE_WORDS],
    /// Sorted by `(start, Reverse(end))`.
    containers: Vec<Container>,
}

impl Facts {
    /// Does the file contain at least one node of this type?
    pub fn contains(&self, ty: AstType) -> bool {
        self.contains_index(ty as usize)
    }

    /// [`Facts::contains`] by `AstType as usize`.
    pub fn contains_index(&self, t: usize) -> bool {
        self.presence[t >> 6] & (1u64 << (t & 63)) != 0
    }

    /// Name of the innermost function unit or class whose span contains `offset`.
    pub fn symbol_at(&self, offset: u32) -> Option<&str> {
        let idx = self.containers.partition_point(|c| c.start <= offset);
        if idx == 0 {
            return None;
        }
        let mut i = (idx - 1) as u32;
        // The innermost container holding `offset` is the candidate with the largest start or
        // one of its ancestors (spans nest properly).
        loop {
            let c = self.containers.get(i as usize)?;
            if offset < c.end {
                return Some(container_name(&self.functions, c));
            }
            if c.parent == NONE {
                return None;
            }
            i = c.parent;
        }
    }
}

enum Role {
    Unit {
        name: String,
        name_span: Span,
        member: bool,
    },
    Inline,
}

/// The name of a container: a function unit's is that of its metric (stored once, there).
fn container_name<'f>(functions: &'f [FunctionMetric], c: &'f Container) -> &'f str {
    if c.metric == NONE {
        &c.name
    } else {
        functions
            .get(c.metric as usize)
            .map_or(c.name.as_str(), |f| f.name.as_str())
    }
}

/// `format!("{a}{sep}{b}")`, allocated once at its final size (built for every function).
fn joined(a: &str, sep: char, b: &str) -> String {
    let mut s = String::with_capacity(a.len() + sep.len_utf8() + b.len());
    s.push_str(a);
    s.push(sep);
    s.push_str(b);
    s
}

pub(crate) fn compute(semantic: &Semantic<'_>, source: &str, lines: &LineIndex<'_>) -> Facts {
    let nodes = semantic.nodes();
    let len = nodes.len();
    // Per node, in node order: (owning container, nesting, depth). Nodes come in pre-order, so a
    // node's parent is already in the list when the node is visited (a parent not yet in it reads
    // as (NONE, 0, 0), as before the list was filled in order).
    let mut per_node: Vec<(u32, u32, u32)> = Vec::with_capacity(len);

    let mut facts = Facts {
        functions: Vec::new(),
        classes: 0,
        statements: 0,
        presence: [0; PRESENCE_WORDS],
        containers: Vec::new(),
    };

    for (id, node) in nodes.iter_enumerated() {
        let kind = node.kind();
        let ty = kind.ty() as usize;
        facts.presence[ty >> 6] |= 1u64 << (ty & 63);
        let i = id.index();
        let span = kind.span();

        let (mut own, mut ns, mut dp) = (NONE, 0u32, 0u32);
        let mut parent_kind = None;
        if i != 0 {
            let pid = nodes.parent_id(id);
            let p = pid.index();
            if let Some(&(o, n, d)) = per_node.get(p) {
                own = o;
                ns = n;
                dp = d;
            }
            let pk = nodes.kind(pid);
            parent_kind = Some(pk);
            // Nesting rises inside the *bodies* of control structures.
            match pk {
                AstKind::IfStatement(s) => {
                    let is_else_if = matches!(kind, AstKind::IfStatement(_));
                    if span == s.consequent.span()
                        || (!is_else_if && s.alternate.as_ref().is_some_and(|a| a.span() == span))
                    {
                        ns += 1;
                        dp += 1;
                    }
                }
                AstKind::ForStatement(s) if s.body.span() == span => {
                    ns += 1;
                    dp += 1;
                }
                AstKind::ForInStatement(s) if s.body.span() == span => {
                    ns += 1;
                    dp += 1;
                }
                AstKind::ForOfStatement(s) if s.body.span() == span => {
                    ns += 1;
                    dp += 1;
                }
                AstKind::WhileStatement(s) if s.body.span() == span => {
                    ns += 1;
                    dp += 1;
                }
                AstKind::DoWhileStatement(s) if s.body.span() == span => {
                    ns += 1;
                    dp += 1;
                }
                AstKind::WithStatement(s) if s.body.span() == span => dp += 1,
                AstKind::SwitchStatement(_) if matches!(kind, AstKind::SwitchCase(_)) => {
                    ns += 1;
                    dp += 1;
                }
                AstKind::TryStatement(_) => dp += 1,
                AstKind::CatchClause(c) if c.body.span == span => ns += 1,
                AstKind::ConditionalExpression(c)
                    if c.consequent.span() == span || c.alternate.span() == span =>
                {
                    ns += 1;
                }
                _ => {}
            }
        }

        let own_metric = |own: u32, containers: &[Container]| {
            if own == NONE {
                NONE
            } else {
                containers.get(own as usize).map_or(NONE, |c| c.metric)
            }
        };

        // Function units and classes.
        let role = match kind {
            AstKind::Function(f) if f.body.is_some() => {
                Some(classify_function(nodes, id, f, source))
            }
            AstKind::ArrowFunctionExpression(a) => Some(classify_expression(
                nodes,
                id,
                a.span,
                None,
                arrow_token_span(a, source),
                source,
            )),
            _ => None,
        };
        if let Some(role) = role {
            let enclosing_metric = own_metric(own, &facts.containers);
            let enclosing = facts.containers.get(own as usize);
            // Directly in a test suite's callback (a container without a metric, see
            // `is_suite_callback`): callbacks there are units of their own, and names are
            // formed as if the suite's callback were still the enclosing unit.
            let in_suite = enclosing.is_some_and(|c| c.suite);
            let outer_name =
                enclosing.map_or("<anonymous>", |c| container_name(&facts.functions, c));
            // The name of a unit (`None`: folded into the enclosing unit).
            let unit = match role {
                Role::Inline if enclosing_metric != NONE => None,
                // Named like the suite: the symbol its code had when it was folded into it.
                Role::Inline if in_suite => {
                    Some((outer_name.to_string(), anonymous_token(kind, source)))
                }
                Role::Inline => Some(("<anonymous>".to_string(), anonymous_token(kind, source))),
                Role::Unit {
                    name,
                    name_span,
                    member,
                } => {
                    let name = if member {
                        joined(outer_name, '#', &name)
                    } else if enclosing_metric != NONE || in_suite {
                        joined(outer_name, '.', &name)
                    } else {
                        name
                    };
                    Some((name, name_span))
                }
            };
            let suite = matches!(kind, AstKind::ArrowFunctionExpression(_))
                || matches!(kind, AstKind::Function(f) if !f.is_declaration());
            let suite = suite && is_suite_callback(semantic, nodes, id, span);
            match unit {
                // A test suite's callback groups tests; it is not a function to measure. It
                // becomes a container like a class, named like the code it holds was named so
                // far (its own name, or that of the unit it was folded into), and the tests and
                // hooks in it become units of their own.
                _ if suite => {
                    let name = match unit {
                        Some((name, _)) => name,
                        None => outer_name.to_string(),
                    };
                    facts.containers.push(Container {
                        name,
                        start: span.start,
                        end: span.end,
                        parent: NONE,
                        metric: NONE,
                        suite: true,
                    });
                    own = (facts.containers.len() - 1) as u32;
                    ns = 0;
                    dp = 0;
                }
                None => ns += 1, // folded callback
                Some((name, name_span)) => {
                    let params = match kind {
                        AstKind::Function(f) => param_count(&f.params),
                        AstKind::ArrowFunctionExpression(a) => param_count(&a.params),
                        _ => 0,
                    };
                    let start = name_span.start.min(span.start);
                    let metric_index = facts.functions.len() as u32;
                    facts.functions.push(FunctionMetric {
                        name,
                        range: lines.range(name_span.start, name_span.end),
                        start_line: lines.line_of(start),
                        end_line: lines.line_of(span.end),
                        cyclomatic: 1,
                        cognitive: 0,
                        params,
                        max_nesting: 0,
                        statements: 0,
                    });
                    facts.containers.push(Container {
                        // Read from the metric (see `container_name`).
                        name: String::new(),
                        start: span.start,
                        end: span.end,
                        parent: NONE,
                        metric: metric_index,
                        suite: false,
                    });
                    own = (facts.containers.len() - 1) as u32;
                    ns = 0;
                    dp = 0;
                }
            }
        } else if let AstKind::Class(c) = kind {
            facts.classes += 1;
            facts.containers.push(Container {
                name: class_name(
                    nodes,
                    id,
                    c.id.as_ref().map(|b| b.name.as_str()),
                    c.span,
                    source,
                ),
                start: c.span.start,
                end: c.span.end,
                parent: NONE,
                metric: NONE,
                suite: false,
            });
            own = (facts.containers.len() - 1) as u32;
        }

        debug_assert_eq!(per_node.len(), i);
        per_node.push((own, ns, dp));

        // Statement counts.
        let is_statement = match kind {
            AstKind::ExpressionStatement(_)
            | AstKind::IfStatement(_)
            | AstKind::ForStatement(_)
            | AstKind::ForInStatement(_)
            | AstKind::ForOfStatement(_)
            | AstKind::WhileStatement(_)
            | AstKind::DoWhileStatement(_)
            | AstKind::ReturnStatement(_)
            | AstKind::BreakStatement(_)
            | AstKind::ContinueStatement(_)
            | AstKind::ThrowStatement(_)
            | AstKind::TryStatement(_)
            | AstKind::SwitchStatement(_)
            | AstKind::DebuggerStatement(_)
            | AstKind::WithStatement(_) => true,
            AstKind::VariableDeclaration(d) => {
                !d.declare
                    && !matches!(
                        parent_kind,
                        Some(
                            AstKind::ForStatement(_)
                                | AstKind::ForInStatement(_)
                                | AstKind::ForOfStatement(_)
                        )
                    )
            }
            _ => false,
        };
        if is_statement {
            facts.statements += 1;
        }

        let metric = own_metric(own, &facts.containers);
        let Some(m) = facts.functions.get_mut(metric as usize) else {
            continue;
        };
        if is_statement {
            m.statements += 1;
        }
        match kind {
            AstKind::IfStatement(s) => {
                let is_else_if = matches!(
                    parent_kind,
                    Some(AstKind::IfStatement(p)) if p.alternate.as_ref().is_some_and(|a| a.span() == span)
                );
                if is_else_if {
                    m.cognitive += 1;
                } else {
                    m.cognitive += 1 + ns;
                    m.max_nesting = m.max_nesting.max(dp + 1);
                }
                m.cyclomatic += 1;
                if s.alternate
                    .as_ref()
                    .is_some_and(|a| !matches!(a, Statement::IfStatement(_)))
                {
                    m.cognitive += 1; // else
                }
            }
            AstKind::ForStatement(_)
            | AstKind::ForInStatement(_)
            | AstKind::ForOfStatement(_)
            | AstKind::WhileStatement(_)
            | AstKind::DoWhileStatement(_) => {
                m.cognitive += 1 + ns;
                m.cyclomatic += 1;
                m.max_nesting = m.max_nesting.max(dp + 1);
            }
            AstKind::SwitchStatement(_) => {
                m.cognitive += 1 + ns;
                m.max_nesting = m.max_nesting.max(dp + 1);
            }
            AstKind::SwitchCase(c) if c.test.is_some() => m.cyclomatic += 1,
            AstKind::TryStatement(_) | AstKind::WithStatement(_) => {
                m.max_nesting = m.max_nesting.max(dp + 1);
            }
            AstKind::CatchClause(_) | AstKind::ConditionalExpression(_) => {
                m.cognitive += 1 + ns;
                m.cyclomatic += 1;
            }
            AstKind::LogicalExpression(e) => {
                m.cyclomatic += 1;
                let continues = matches!(
                    parent_kind,
                    Some(AstKind::LogicalExpression(p)) if p.operator == e.operator
                );
                if !continues {
                    m.cognitive += 1;
                }
            }
            AstKind::AssignmentExpression(a) if a.operator.is_logical() => m.cyclomatic += 1,
            AstKind::BreakStatement(b) if b.label.is_some() => m.cognitive += 1,
            AstKind::ContinueStatement(c) if c.label.is_some() => m.cognitive += 1,
            _ => {}
        }
    }

    finish_containers(&mut facts.containers);
    facts
}

/// Sort containers by span and link each to its innermost enclosing container.
fn finish_containers(containers: &mut [Container]) {
    containers.sort_by(|a, b| a.start.cmp(&b.start).then(b.end.cmp(&a.end)));
    let mut stack: Vec<u32> = Vec::new();
    for i in 0..containers.len() {
        let start = containers[i].start;
        while let Some(&top) = stack.last() {
            if containers[top as usize].end > start {
                break;
            }
            stack.pop();
        }
        containers[i].parent = stack.last().copied().unwrap_or(NONE);
        stack.push(i as u32);
    }
}

fn param_count(params: &oxc_ast::ast::FormalParameters<'_>) -> u32 {
    (params.items.len() + usize::from(params.rest.is_some())) as u32
}

fn classify_function<'a>(nodes: &AstNodes<'a>, id: NodeId, f: &Function<'a>, source: &str) -> Role {
    if f.is_declaration() {
        return match &f.id {
            Some(b) => Role::Unit {
                name: b.name.to_string(),
                name_span: b.span,
                member: false,
            },
            None => Role::Unit {
                name: "default".to_string(),
                name_span: function_keyword_span(f, source),
                member: false,
            },
        };
    }
    classify_expression(
        nodes,
        id,
        f.span,
        f.id.as_ref().map(|b| (b.name.as_str(), b.span)),
        function_keyword_span(f, source),
        source,
    )
}

/// Classify a function/arrow *expression* by where it appears.
fn classify_expression<'a>(
    nodes: &AstNodes<'a>,
    id: NodeId,
    span: Span,
    own_name: Option<(&str, Span)>,
    token: Span,
    source: &str,
) -> Role {
    let (_, parent, child_span) = transparent_parent(nodes, id, span);
    let unit = |name: String, name_span: Span, member: bool| Role::Unit {
        name,
        name_span,
        member,
    };
    match parent {
        Some(AstKind::MethodDefinition(m)) if m.value.span == child_span => {
            unit(key_name(&m.key, source), m.key.span(), true)
        }
        Some(AstKind::PropertyDefinition(p))
            if p.value.as_ref().is_some_and(|v| v.span() == child_span) =>
        {
            unit(key_name(&p.key, source), p.key.span(), true)
        }
        Some(AstKind::AccessorProperty(p))
            if p.value.as_ref().is_some_and(|v| v.span() == child_span) =>
        {
            unit(key_name(&p.key, source), p.key.span(), true)
        }
        Some(AstKind::ObjectProperty(p)) if p.value.span() == child_span => {
            let pid = transparent_parent(nodes, id, span).0;
            let key = key_name(&p.key, source);
            let name = match object_owner_name(nodes, pid, source, 0) {
                Some(owner) => joined(&owner, '.', &key),
                None => key,
            };
            unit(name, p.key.span(), false)
        }
        Some(AstKind::VariableDeclarator(d))
            if d.init.as_ref().is_some_and(|v| v.span() == child_span) =>
        {
            match &d.id {
                BindingPattern::BindingIdentifier(b) => unit(b.name.to_string(), b.span, false),
                _ => unit("<anonymous>".to_string(), token, false),
            }
        }
        Some(AstKind::AssignmentExpression(a)) if a.right.span() == child_span => unit(
            assignment_target_name(&a.left, source),
            a.left.span(),
            false,
        ),
        Some(AstKind::ExportDefaultDeclaration(_)) => match own_name {
            Some((name, name_span)) => unit(name.to_string(), name_span, false),
            None => unit("default".to_string(), token, false),
        },
        _ => match own_name {
            Some((name, name_span)) => unit(name.to_string(), name_span, false),
            None => Role::Inline,
        },
    }
}

/// Test-framework functions whose callback groups tests: `describe`, `context` and `suite`
/// (Mocha, Jest, Vitest, Jasmine, `node:test`) and their focused and skipped forms.
const SUITE_FUNCTIONS: &[&str] = &[
    "context",
    "describe",
    "fdescribe",
    "suite",
    "xcontext",
    "xdescribe",
];

/// Modifiers of a suite call: `describe.only(…)`, `describe.each(table)(…)`, `describe.for(…)(…)`,
/// `test.describe.serial(…)`, `describe.skipIf(cond)(…)`, …
const SUITE_MODIFIERS: &[&str] = &[
    "concurrent",
    "each",
    "fixme",
    "for",
    "only",
    "parallel",
    "runIf",
    "sequential",
    "serial",
    "shuffle",
    "skip",
    "skipIf",
    "todo",
];

/// Whether the function expression `id` (spanning `span`) is an argument of a test-suite call:
/// `describe('…', () => { … })`, `describe.each(table)('…', (a) => { … })`,
/// `test.describe.serial('…', () => { … })`. The callee's root must be a test framework's API: a
/// global (no declaration in the file) or an import. `schema.describe(…)`, a local function
/// named `context`, `test.describe.configure(…)` are not suites.
fn is_suite_callback(
    semantic: &Semantic<'_>,
    nodes: &AstNodes<'_>,
    id: NodeId,
    span: Span,
) -> bool {
    let (_, parent, child_span) = transparent_parent(nodes, id, span);
    let Some(AstKind::CallExpression(call)) = parent else {
        return false;
    };
    if !call.arguments.iter().any(|a| a.span() == child_span) {
        return false;
    }
    // Walk the callee from the outside in: `test.describe.only` visits `only`, `describe`, then
    // the root `test`.
    let mut current = call.callee.get_inner_expression();
    let mut next_to_root = None;
    let mut describes = 0;
    loop {
        match current {
            Expression::Identifier(root) => {
                let suite = match root.name.as_str() {
                    "test" => describes == 1 && next_to_root == Some("describe"),
                    name => describes == 0 && SUITE_FUNCTIONS.contains(&name),
                };
                return suite && is_test_api(semantic, root);
            }
            Expression::StaticMemberExpression(m) => {
                let name = m.property.name.as_str();
                if name == "describe" {
                    describes += 1;
                } else if !SUITE_MODIFIERS.contains(&name) {
                    return false;
                }
                next_to_root = Some(name);
                current = m.object.get_inner_expression();
            }
            // `describe.each(table)(…)`, `describe.each`…`(…)`.
            Expression::CallExpression(c) => current = c.callee.get_inner_expression(),
            Expression::TaggedTemplateExpression(t) => current = t.tag.get_inner_expression(),
            _ => return false,
        }
    }
}

/// Whether `id` refers to a test framework's API: an unresolved global, an import, or a `const`
/// taken from a `require(…)` (`const { describe, it } = require('node:test')`).
fn is_test_api(semantic: &Semantic<'_>, id: &IdentifierReference<'_>) -> bool {
    let Some(reference) = id.reference_id.get() else {
        return false;
    };
    let scoping = semantic.scoping();
    let Some(symbol) = scoping.get_reference(reference).symbol_id() else {
        return true;
    };
    let flags = scoping.symbol_flags(symbol);
    if flags.contains(SymbolFlags::Import) {
        return true;
    }
    if !flags.contains(SymbolFlags::ConstVariable) {
        return false;
    }
    let AstKind::VariableDeclarator(d) = semantic.symbol_declaration(symbol).kind() else {
        return false;
    };
    // `require('m')`, or a member of it (`require('node:test').describe`).
    let mut init = d.init.as_ref().map(Expression::get_inner_expression);
    while let Some(Expression::StaticMemberExpression(m)) = init {
        init = Some(m.object.get_inner_expression());
    }
    matches!(init, Some(Expression::CallExpression(call))
        if call.arguments.len() == 1
            && matches!(call.arguments[0].as_expression(), Some(Expression::StringLiteral(_)))
            && matches!(&call.callee, Expression::Identifier(r) if r.name == "require"
                && r.reference_id.get().is_some_and(|rid| scoping.get_reference(rid).symbol_id().is_none())))
}

/// First ancestor that is not a type-only expression wrapper. Returns its id, kind, and the span
/// of the child through which it was reached.
fn transparent_parent<'a>(
    nodes: &AstNodes<'a>,
    id: NodeId,
    span: Span,
) -> (NodeId, Option<AstKind<'a>>, Span) {
    let mut child = id;
    let mut child_span = span;
    for _ in 0..64 {
        if child == NodeId::ROOT {
            return (child, None, child_span);
        }
        let pid = nodes.parent_id(child);
        let pk = nodes.kind(pid);
        match pk {
            AstKind::TSAsExpression(_)
            | AstKind::TSSatisfiesExpression(_)
            | AstKind::TSNonNullExpression(_)
            | AstKind::TSTypeAssertion(_)
            | AstKind::ParenthesizedExpression(_) => {
                child = pid;
                child_span = pk.span();
            }
            _ => return (pid, Some(pk), child_span),
        }
    }
    (child, None, child_span)
}

fn class_name<'a>(
    nodes: &AstNodes<'a>,
    id: NodeId,
    own: Option<&str>,
    span: Span,
    source: &str,
) -> String {
    if let Some(name) = own {
        return name.to_string();
    }
    match transparent_parent(nodes, id, span) {
        (_, Some(AstKind::VariableDeclarator(d)), _) => match &d.id {
            BindingPattern::BindingIdentifier(b) => b.name.to_string(),
            _ => "<anonymous>".to_string(),
        },
        (_, Some(AstKind::ExportDefaultDeclaration(_)), _) => "default".to_string(),
        (_, Some(AstKind::AssignmentExpression(a)), _) => assignment_target_name(&a.left, source),
        (_, Some(AstKind::PropertyDefinition(p)), _) => key_name(&p.key, source),
        (_, Some(AstKind::ObjectProperty(p)), _) => key_name(&p.key, source),
        _ => "<anonymous>".to_string(),
    }
}

/// Name prefix for members of an object literal: `obj` in `const obj = { run() {} }`.
fn object_owner_name<'a>(
    nodes: &AstNodes<'a>,
    object_property: NodeId,
    source: &str,
    level: u32,
) -> Option<String> {
    if level > 4 || object_property == NodeId::ROOT {
        return None;
    }
    let object = nodes.parent_id(object_property);
    let AstKind::ObjectExpression(obj) = nodes.kind(object) else {
        return None;
    };
    match transparent_parent(nodes, object, obj.span) {
        (_, Some(AstKind::VariableDeclarator(d)), child)
            if d.init.as_ref().is_some_and(|v| v.span() == child) =>
        {
            match &d.id {
                BindingPattern::BindingIdentifier(b) => Some(b.name.to_string()),
                _ => None,
            }
        }
        (_, Some(AstKind::AssignmentExpression(a)), child) if a.right.span() == child => {
            Some(assignment_target_name(&a.left, source))
        }
        (pid, Some(AstKind::ObjectProperty(p)), child) if p.value.span() == child => {
            let key = key_name(&p.key, source);
            Some(match object_owner_name(nodes, pid, source, level + 1) {
                Some(outer) => joined(&outer, '.', &key),
                None => key,
            })
        }
        (_, Some(AstKind::ExportDefaultDeclaration(_)), _) => Some("default".to_string()),
        _ => None,
    }
}

/// Display name of a property key: `name`, `#private`, `'literal'` value, or `[computed]`.
pub(crate) fn key_name(key: &PropertyKey<'_>, source: &str) -> String {
    match key {
        PropertyKey::PrivateIdentifier(p) => format!("#{}", p.name),
        _ => match key.static_name() {
            Some(name) => name.into_owned(),
            None => format!("[{}]", compact_text(source, key.span())),
        },
    }
}

fn assignment_target_name(target: &AssignmentTarget<'_>, source: &str) -> String {
    match target {
        AssignmentTarget::AssignmentTargetIdentifier(id) => id.name.to_string(),
        AssignmentTarget::StaticMemberExpression(m) => {
            if is_simple_chain(&m.object) {
                compact_text(source, m.span)
            } else {
                m.property.name.to_string()
            }
        }
        _ => match target.as_simple_assignment_target() {
            Some(SimpleAssignmentTarget::PrivateFieldExpression(p)) => format!("#{}", p.field.name),
            _ => "<anonymous>".to_string(),
        },
    }
}

fn is_simple_chain(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::Identifier(_) | Expression::ThisExpression(_) => true,
        Expression::StaticMemberExpression(m) => is_simple_chain(&m.object),
        _ => false,
    }
}

/// Source text of `span` with whitespace removed, clipped to a sane length.
fn compact_text(source: &str, span: Span) -> String {
    let text = source
        .get(span.start as usize..span.end as usize)
        .unwrap_or("");
    let mut out: String = text
        .chars()
        .filter(|c| !c.is_whitespace())
        .take(60)
        .collect();
    if out.is_empty() {
        out.push('?');
    }
    out
}

fn function_keyword_span(f: &Function<'_>, source: &str) -> Span {
    let end = f.params.span.start.min(f.span.end).max(f.span.start);
    if let Some(text) = source.get(f.span.start as usize..end as usize) {
        if let Some(pos) = text.find("function") {
            let start = f.span.start + pos as u32;
            return Span::new(start, start + 8);
        }
    }
    Span::new(f.span.start, f.span.start)
}

fn arrow_token_span(a: &oxc_ast::ast::ArrowFunctionExpression<'_>, source: &str) -> Span {
    let from = a
        .return_type
        .as_ref()
        .map_or(a.params.span.end, |r| r.span.end);
    let to = a.body.span.start.max(from);
    if let Some(text) = source.get(from as usize..to as usize) {
        if let Some(pos) = text.find("=>") {
            let start = from + pos as u32;
            return Span::new(start, start + 2);
        }
    }
    Span::new(a.span.start, a.span.start)
}

fn anonymous_token(kind: AstKind<'_>, source: &str) -> Span {
    match kind {
        AstKind::Function(f) => function_keyword_span(f, source),
        AstKind::ArrowFunctionExpression(a) => arrow_token_span(a, source),
        _ => Span::new(kind.span().start, kind.span().start),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxc_allocator::Allocator;
    use oxc_parser::{ParseOptions, Parser};
    use oxc_semantic::SemanticBuilder;
    use oxc_span::SourceType;

    fn facts_of(source: &str, st: SourceType) -> Facts {
        let alloc = Allocator::default();
        let ret = Parser::new(&alloc, source, st)
            .with_options(ParseOptions {
                preserve_parens: false,
                ..ParseOptions::default()
            })
            .parse();
        assert!(ret.errors.is_empty(), "parse errors: {:?}", ret.errors);
        let semantic = SemanticBuilder::new().build(&ret.program).semantic;
        let lines = LineIndex::new(source);
        compute(&semantic, source, &lines)
    }

    fn js(source: &str) -> Facts {
        facts_of(source, SourceType::mjs().with_jsx(true))
    }

    fn metric(facts: &Facts, name: &str) -> FunctionMetric {
        facts
            .functions
            .iter()
            .find(|f| f.name == name)
            .cloned()
            .unwrap_or_else(|| panic!("no function {name} in {:?}", names(facts)))
    }

    fn names(facts: &Facts) -> Vec<&str> {
        facts.functions.iter().map(|f| f.name.as_str()).collect()
    }

    #[test]
    fn empty_function_has_base_values() {
        let f = js("function a(x, y, ...z) {}\n");
        let m = metric(&f, "a");
        assert_eq!(
            (
                m.cyclomatic,
                m.cognitive,
                m.params,
                m.max_nesting,
                m.statements
            ),
            (1, 0, 3, 0, 0)
        );
        assert_eq!((m.start_line, m.end_line), (1, 1));
        assert_eq!((m.range.start_col, m.range.end_col), (10, 11));
    }

    #[test]
    fn if_else_chain() {
        // if (+1), else if (+1), else (+1), && (+1), nested if (+2)
        let src = "function f(a, b) {\n  if (a && b) {\n    if (b) { return 1; }\n  } else if (b) {\n    return 2;\n  } else {\n    return 3;\n  }\n  return 0;\n}\n";
        let m = metric(&js(src), "f");
        assert_eq!(m.cognitive, 1 + 1 + 2 + 1 + 1);
        // 1 + if + && + if + else-if
        assert_eq!(m.cyclomatic, 5);
        assert_eq!(m.max_nesting, 2);
        assert_eq!(m.statements, 7);
        assert_eq!((m.start_line, m.end_line), (1, 10));
    }

    #[test]
    fn loops_switch_catch_ternary() {
        let src = r"
function g(xs) {
  for (const x of xs) {          // +1
    while (x) {                  // +2
      switch (x) {               // +3
        case 1: break;
        case 2: continue;
        default: break;
      }
    }
  }
  try {
    run();
  } catch (e) {                  // +1
    const y = e ? 1 : 2;         // +2
  }
}
";
        let m = metric(&js(src), "g");
        assert_eq!(m.cognitive, 1 + 2 + 3 + 1 + 2);
        // 1 + for + while + 2 cases + catch + ternary
        assert_eq!(m.cyclomatic, 7);
        assert_eq!(m.max_nesting, 3);
    }

    #[test]
    fn logical_sequences() {
        let src = "function h(a, b, c, d, e) { return (a && b && c || d && e) ?? a; }\n";
        let m = metric(&js(src), "h");
        // `&&` run, `||`, `&&` run, `??` => 4
        assert_eq!(m.cognitive, 4);
        assert_eq!(m.cyclomatic, 1 + 5);
        let m2 = metric(&js("function k(a, b, c) { return a && (b && c); }\n"), "k");
        assert_eq!(m2.cognitive, 1);
        let m3 = metric(&js("function k(a, b, c) { return a && !(b && c); }\n"), "k");
        assert_eq!(m3.cognitive, 2);
    }

    #[test]
    fn labelled_jumps() {
        let src = "function j(xs) { outer: for (const a of xs) { for (const b of a) { if (b) continue outer; if (!b) break outer; } } }\n";
        let m = metric(&js(src), "j");
        // for +1, inner for +2, two ifs +3 each, two labelled jumps +1 each
        assert_eq!(m.cognitive, 1 + 2 + 3 + 3 + 1 + 1);
    }

    #[test]
    fn callbacks_fold_into_enclosing_unit() {
        let src = r"
function outer(items) {
  items.forEach((item) => {      // nesting +1
    if (item) {                  // +2
      log(item);
    }
  });
  return items.map(function (x) { return x ? 1 : 0; }); // ternary +2
}
";
        let f = js(src);
        assert_eq!(names(&f), vec!["outer"]);
        let m = metric(&f, "outer");
        assert_eq!(m.cognitive, 2 + 2);
        assert_eq!(m.cyclomatic, 3);
        assert_eq!(m.max_nesting, 1);
        assert_eq!(m.statements, 5);
    }

    #[test]
    fn named_and_assigned_functions_are_separate() {
        let src = r"
class Shop {
  constructor() { this.items = []; }
  get size() { return this.items.length; }
  total = () => this.items.reduce((a, b) => a + b, 0);
  static #helper(x) { if (x) { return 1; } return 0; }
}
const obj = { run() { if (a) {} }, nested: { deep: function () {} } };
function top() {
  function inner() { if (x) {} }
  const local = () => { while (y) {} };
  return inner;
}
exports.handler = async function () {};
export default function () {}
describe('suite', () => { it('works', () => { if (z) {} }); });
";
        let f = js(src);
        assert_eq!(
            names(&f),
            vec![
                "Shop#constructor",
                "Shop#size",
                "Shop#total",
                "Shop##helper",
                "obj.run",
                "obj.nested.deep",
                "top",
                "top.inner",
                "top.local",
                "exports.handler",
                "default",
                "<anonymous>",
            ]
        );
        let helper = metric(&f, "Shop##helper");
        assert_eq!(
            (helper.cognitive, helper.cyclomatic, helper.params),
            (1, 2, 1)
        );
        let top = metric(&f, "top");
        assert_eq!((top.cognitive, top.cyclomatic, top.statements), (0, 1, 2));
        let anon = metric(&f, "<anonymous>");
        // The test's callback, not the suite's (a container): `if` at nesting 0 => +1.
        assert_eq!(anon.cognitive, 1);
        assert_eq!(anon.range.start_line, 16);
    }

    #[test]
    fn typescript_bodiless_functions_are_skipped() {
        let src = r"
abstract class A {
  abstract run(): void;
  overload(a: string): void;
  overload(a: any): void { if (a) {} }
}
declare function ambient(x: number): void;
function typed<T>(x: T): T { return x; }
";
        let f = facts_of(src, SourceType::ts());
        assert_eq!(names(&f), vec!["A#overload", "typed"]);
        assert_eq!(f.classes, 1);
    }

    #[test]
    fn file_counts_and_symbols() {
        let src = "const a = 1;\nclass C { m() { return 1; } }\nfunction f() { let x = 2; x++; }\n";
        let f = js(src);
        assert_eq!(f.classes, 1);
        // const, return, let, x++
        assert_eq!(f.statements, 4);
        assert!(f.contains(AstType::Class));
        assert!(!f.contains(AstType::JSXElement));
        let at = |needle: &str| f.symbol_at(src.find(needle).unwrap() as u32);
        assert_eq!(at("const a"), None);
        assert_eq!(at("return 1"), Some("C#m"));
        assert_eq!(at("m()"), Some("C"));
        assert_eq!(at("x++"), Some("f"));
    }

    #[test]
    fn nesting_in_conditions_is_not_increased() {
        // The ternary in the `if` test sits at the `if`'s own level.
        let src = "function t(a, b) { if (a ? b : !b) { return 1; } return 0; }\n";
        let m = metric(&js(src), "t");
        assert_eq!(m.cognitive, 1 + 1);
    }

    /// `(name, start line, end line)` of every unit.
    fn units(facts: &Facts) -> Vec<(&str, u32, u32)> {
        facts
            .functions
            .iter()
            .map(|f| (f.name.as_str(), f.start_line, f.end_line))
            .collect()
    }

    #[test]
    fn suite_callbacks_are_containers_and_their_tests_units() {
        let src = r"
describe('cart', () => {
  let cart;
  const fill = (n) => { for (let i = 0; i < n; i++) cart.add(i); };
  beforeEach(() => {
    cart = new Cart();
  });
  it('adds', () => {
    if (cart) {
      fill(2);
    }
  });
  describe.each([[1], [2]])('with %i items', (n) => {
    test('counts', async function () {
      expect(cart.size).toBe(n);
    });
  });
  context.only('empty', function () {
    it.skip('is empty', () => {});
  });
});
";
        let f = js(src);
        assert_eq!(
            units(&f),
            vec![
                ("<anonymous>.fill", 4, 4),
                ("<anonymous>", 5, 7),
                ("<anonymous>", 8, 12),
                ("<anonymous>", 14, 16),
                ("<anonymous>", 19, 19),
            ],
            "no unit for the three suites; the hook, the tests and the helper are units"
        );
        // The test's complexity is its own: `if` at nesting 0, not inside a folded callback.
        assert_eq!(f.functions[2].cognitive, 1);
        assert_eq!(f.functions[2].max_nesting, 1);
        // Symbols as before: the suite's callback was `<anonymous>`, so is everything in it.
        let at = |needle: &str| f.symbol_at(src.find(needle).unwrap() as u32);
        assert_eq!(at("let cart"), Some("<anonymous>"));
        assert_eq!(at("fill(2)"), Some("<anonymous>"));
        assert_eq!(at("i++"), Some("<anonymous>.fill"));
        assert_eq!(at("expect("), Some("<anonymous>"));
    }

    #[test]
    fn suites_of_other_apis_and_forms() {
        // Imported APIs (Vitest, Playwright), a named suite callback, a suite inside a function:
        // names stay those the code had before. An import under another name is not recognized
        // (its callback stays a unit, the test in it folded).
        let src = r"
import { describe, it } from 'vitest';
import { test } from '@playwright/test';
import { describe as group } from 'node:test';
describe('a', () => {
  it('x', () => {});
});
test.describe.serial('b', () => {
  test('y', async ({ page }) => { await page.goto('/'); });
});
test.describe(() => {
  test.beforeEach(async () => {});
});
describe('c', function named() {
  it('z', function () {});
});
function shared(kind) {
  describe(kind, () => {
    it('w', () => {});
  });
}
group('d', () => {
  it('v', () => {});
});
";
        let f = facts_of(src, SourceType::ts());
        assert_eq!(
            units(&f),
            vec![
                ("<anonymous>", 6, 6),
                ("<anonymous>", 9, 9),
                ("<anonymous>", 12, 12),
                ("named", 15, 15),
                ("shared", 17, 21),
                ("shared", 19, 19),
                ("<anonymous>", 22, 24),
            ]
        );
    }

    #[test]
    fn suites_of_required_apis() {
        // CommonJS: `node:test` through `require`, destructured or as a member; `describe.for`.
        let src = r"
const { describe, it } = require('node:test');
const suite = require('mocha').suite;
describe('a', () => {
  it('x', () => {});
});
suite('b', () => {
  it('y', () => {});
});
describe.for([1, 2])('c %i', (n) => {
  it('z', () => {});
});
";
        let f = js(src);
        assert_eq!(
            units(&f),
            vec![
                ("<anonymous>", 5, 5),
                ("<anonymous>", 8, 8),
                ("<anonymous>", 11, 11)
            ]
        );
        // A `const` with another value is not a test API, nor is `require` declared in the file.
        let src = r"
const context = makeContext();
context('a', () => {
  it('x', () => {});
});
function load(require) {
  const { describe } = require('node:test');
  describe('b', () => {
    it('y', () => {});
  });
}
";
        let f = js(src);
        assert_eq!(units(&f), vec![("<anonymous>", 3, 5), ("load", 6, 11)]);
    }

    #[test]
    fn focused_skipped_and_table_suites() {
        let src = r"
fdescribe('a', () => {
  it('x', () => {});
});
xdescribe('b', () => {
  it('y', () => {});
});
describe.skip('c', () => {
  it('z', () => {});
});
describe.each`
  a | b
  ${1} | ${2}
`('d $a', ({ a, b }) => {
  it('w', () => {});
});
test.describe.parallel.only('e', () => {
  test('v', () => {});
});
";
        let f = js(src);
        assert_eq!(
            units(&f),
            vec![
                ("<anonymous>", 3, 3),
                ("<anonymous>", 6, 6),
                ("<anonymous>", 9, 9),
                ("<anonymous>", 15, 15),
                ("<anonymous>", 18, 18),
            ]
        );
    }

    #[test]
    fn calls_that_are_not_suites_fold_as_before() {
        let src = r"
function f(schema, describe2) {
  schema.describe(() => { if (a) { b(); } });
  describe2('x', () => { if (a) { b(); } });
  test.describe.configure(() => { if (a) { b(); } });
  context('x').run(() => { if (a) { b(); } });
}
function g() {
  const context = makeContext();
  context('x', () => { if (a) { b(); } });
}
function describe(name, fn) { fn(); }
function h() {
  describe('x', () => { if (a) { b(); } });
}
";
        let f = js(src);
        assert_eq!(names(&f), vec!["f", "g", "describe", "h"]);
        // Each callback is folded: its `if` counts towards the enclosing function with the
        // nesting a callback adds.
        assert_eq!(metric(&f, "f").cognitive, 4 * 2);
        assert_eq!(metric(&f, "g").cognitive, 2);
        assert_eq!(metric(&f, "h").cognitive, 2);
    }
}
