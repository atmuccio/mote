//! `js/no-identical-functions` — during the traversal, each candidate body gets a cheap shallow
//! key (statement kinds and a few names; no scan of the body). `run_once` hashes only bodies that
//! share a key with another one, and confirms equal hashes with a structural AST comparison.

use crate::prelude::*;
use oxc_span::ContentEq;
use rustc_hash::FxHasher;
use std::hash::{Hash, Hasher};

pub struct NoIdenticalFunctions;

pub static NO_IDENTICAL_FUNCTIONS: RuleMeta = RuleMeta {
    severity: Severity::Medium,
    tags: &["duplication"],
    remediation_minutes: 15,
    params: &[ParamSpec {
        name: "min_lines",
        default: ParamDefault::Int(3),
        description: "Minimum number of lines spanned by a body's statements.",
    }],
    description: r#"Flags a function, method, or arrow function whose body is identical to the body of
another function in the same file. Bodies are compared structurally, so formatting, comments,
quote style, and optional semicolons or parentheses do not matter. Only bodies whose statements
span at least 3 lines (`min_lines`) are considered, and two closures only count as identical when
their names refer to the same variables (callbacks capturing different loop variables are not
interchangeable). Bodies that consist of a single `throw` ("not supported" stubs) and test files
are not checked.

**Why:** duplicated logic has to be fixed and changed in several places; sooner or later the
copies drift apart and one of them keeps the bug.

**Fix:** keep one implementation and call it from the other place, or extract the shared body
into a helper that both use.

```js
function totalPrice(items) {
  let sum = 0;
  for (const i of items) sum += i.price * i.qty;
  return sum;
}
function orderValue(items) {        // flagged: same body as totalPrice
  let sum = 0;
  for (const i of items) sum += i.price * i.qty;
  return sum;
}

const orderValue = totalPrice;      // compliant
```
"#,
    ..RuleMeta::new(
        "js/no-identical-functions",
        "js",
        "Functions should not have identical implementations",
    )
};

struct Entry {
    node: NodeId,
    body: Span,
    key: u64,
}

#[derive(Default)]
struct Bodies {
    min_lines: Option<u32>,
    entries: Vec<Entry>,
}

fn body_of<'a>(ctx: &Ctx<'a>, id: NodeId) -> Option<&'a FunctionBody<'a>> {
    match ctx.node(id).kind() {
        AstKind::Function(f) => f.body.as_deref(),
        AstKind::ArrowFunctionExpression(a) => Some(&a.body),
        _ => None,
    }
}

/// What a name used in a function body refers to.
#[derive(PartialEq, Eq)]
enum Binding {
    /// A global or unresolved name.
    Global,
    /// Declared inside the function (a parameter or a local).
    Local,
    /// A variable of an enclosing scope, captured by the function.
    Outer(SymbolId),
}

/// The bindings of every identifier reference in the function's body, in source order. Two
/// structurally equal bodies are only the same function when they refer to the same outer
/// variables: two closures over different loop variables that happen to share a name are not
/// interchangeable. Only computed for bodies already known to be equal.
fn bindings(ctx: &Ctx<'_>, function: NodeId) -> Vec<Binding> {
    let Some(body) = body_of(ctx, function) else {
        return Vec::new();
    };
    let own = ctx.node(function).span();
    let span = body.span;
    ctx.nodes()
        .iter()
        .skip(body.node_id.get().index() + 1)
        .take_while(|n| {
            let s = n.span();
            span.start <= s.start && s.end <= span.end
        })
        .filter_map(|n| match n.kind() {
            AstKind::IdentifierReference(ident) => Some(match ctx.reference_symbol(ident) {
                None => Binding::Global,
                Some(symbol) => {
                    let at = ctx.symbol_span(symbol).start;
                    if own.start <= at && at < own.end {
                        Binding::Local
                    } else {
                        Binding::Outer(symbol)
                    }
                }
            }),
            _ => None,
        })
        .collect()
}

/// Where to report a function, and its name when it has one.
fn label<'a>(ctx: &Ctx<'a>, id: NodeId) -> (Span, Option<Cow<'a, str>>) {
    let node = ctx.node(id);
    if let AstKind::Function(f) = node.kind() {
        if let Some(ident) = &f.id {
            return (ident.span, Some(Cow::Borrowed(ident.name.as_str())));
        }
    }
    let named = match ctx.outer_parent(node).map(AstNode::kind) {
        Some(AstKind::MethodDefinition(m)) => Some((m.key.span(), m.key.static_name())),
        Some(AstKind::ObjectProperty(p)) => Some((p.key.span(), p.key.static_name())),
        Some(AstKind::PropertyDefinition(p)) => Some((p.key.span(), p.key.static_name())),
        Some(AstKind::VariableDeclarator(d)) => match &d.id {
            BindingPattern::BindingIdentifier(ident) => {
                Some((ident.span, Some(Cow::Borrowed(ident.name.as_str()))))
            }
            _ => None,
        },
        _ => None,
    };
    if let Some(named) = named {
        return named;
    }
    let start = node.span().start;
    let end = body_of(ctx, id).map_or(start, |b| b.span.start).max(start);
    (Span::new(start, end), None)
}

impl JsRule for NoIdenticalFunctions {
    fn meta(&self) -> &'static RuleMeta {
        &NO_IDENTICAL_FUNCTIONS
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::Function, AstType::ArrowFunctionExpression]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        // Tests favor explicit, self-contained cases over shared helpers.
        !ctx.is_test()
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let body = match node.kind() {
            AstKind::Function(f) if !f.declare => match &f.body {
                Some(body) => body,
                None => return,
            },
            AstKind::ArrowFunctionExpression(a) if !a.expression => &a.body,
            _ => return,
        };
        let (Some(first), Some(last)) = (body.statements.first(), body.statements.last()) else {
            return;
        };
        // `{ throw new Error('not supported'); }` stubs are meant to be repeated.
        if let [Statement::ThrowStatement(_)] = body.statements.as_slice() {
            return;
        }
        let lines = ctx
            .line_of(last.span().end)
            .saturating_sub(ctx.line_of(first.span().start))
            + 1;
        let min_lines = match ctx.state::<Bodies>().min_lines {
            Some(n) => n,
            None => {
                let n = ctx.param_i64("min_lines").clamp(1, i64::from(u32::MAX)) as u32;
                ctx.state::<Bodies>().min_lines = Some(n);
                n
            }
        };
        if lines < min_lines {
            return;
        }
        let key = shallow_key(body);
        ctx.state::<Bodies>().entries.push(Entry {
            node: node.id(),
            body: body.span,
            key,
        });
    }

    fn run_once<'a>(&self, ctx: &mut Ctx<'a>) {
        let mut entries = std::mem::take(&mut ctx.state::<Bodies>().entries);
        if entries.len() < 2 {
            return;
        }
        // Source order within a key, so the first of a group is the original.
        entries.sort_by_key(|e| (e.key, e.body.start));
        let mut reports: Vec<(usize, usize)> = Vec::new();
        let mut hashed: Vec<(u64, usize)> = Vec::new();
        let mut reps: Vec<usize> = Vec::new();
        let mut start = 0;
        while start < entries.len() {
            let key = entries[start].key;
            let len = entries[start..].iter().take_while(|e| e.key == key).count();
            let end = start + len.max(1);
            let group = start..end;
            start = end;
            if len < 2 {
                continue;
            }
            hashed.clear();
            hashed.extend(group.map(|i| (text_hash(ctx, entries[i].body), i)));
            hashed.sort_unstable();
            for same in hashed.chunk_by(|a, b| a.0 == b.0) {
                if same.len() < 2 {
                    continue;
                }
                reps.clear();
                for &(_, i) in same {
                    let Some(body) = body_of(ctx, entries[i].node) else {
                        continue;
                    };
                    let original = reps.iter().copied().find(|&r| {
                        body_of(ctx, entries[r].node).is_some_and(|other| other.content_eq(body))
                            && bindings(ctx, entries[r].node) == bindings(ctx, entries[i].node)
                    });
                    match original {
                        Some(r) => reports.push((i, r)),
                        None => reps.push(i),
                    }
                }
            }
        }
        for (duplicate, original) in reports {
            let (span, _) = label(ctx, entries[duplicate].node);
            let (original_span, original_name) = label(ctx, entries[original].node);
            let line = ctx.line_of(original_span.start);
            let message = match original_name {
                Some(name) => format!(
                    "Reuse '{name}' (line {line}) instead of duplicating its implementation here"
                ),
                None => format!(
                    "Reuse the function on line {line} instead of duplicating its implementation here"
                ),
            };
            ctx.emit(span, message)
                .secondary(original_span, "Original implementation");
        }
    }
}

/// A cheap fingerprint of a body from its top-level statements: their kinds and a few names and
/// literal values. Structurally equal bodies always get equal keys; most different bodies get
/// different ones, so only a few bodies need their full text hashed.
fn shallow_key(body: &FunctionBody<'_>) -> u64 {
    let mut h = FxHasher::default();
    body.statements.len().hash(&mut h);
    for stmt in &body.statements {
        std::mem::discriminant(stmt).hash(&mut h);
        match stmt {
            Statement::ExpressionStatement(s) => expression_key(&s.expression, &mut h, 2),
            Statement::ReturnStatement(s) => {
                if let Some(argument) = &s.argument {
                    expression_key(argument, &mut h, 2);
                }
            }
            Statement::ThrowStatement(s) => expression_key(&s.argument, &mut h, 2),
            Statement::IfStatement(s) => expression_key(&s.test, &mut h, 2),
            Statement::VariableDeclaration(d) => {
                d.declarations.len().hash(&mut h);
                for declarator in &d.declarations {
                    if let BindingPattern::BindingIdentifier(id) = &declarator.id {
                        id.name.as_str().hash(&mut h);
                    }
                    if let Some(init) = &declarator.init {
                        expression_key(init, &mut h, 2);
                    }
                }
            }
            _ => {}
        }
    }
    h.finish()
}

fn expression_key(expr: &Expression<'_>, h: &mut FxHasher, depth: u8) {
    std::mem::discriminant(expr).hash(h);
    match expr {
        Expression::Identifier(id) => id.name.as_str().hash(h),
        Expression::StringLiteral(s) => s.value.as_str().hash(h),
        Expression::StaticMemberExpression(m) => m.property.name.as_str().hash(h),
        Expression::CallExpression(c) => {
            c.arguments.len().hash(h);
            if depth > 0 {
                expression_key(&c.callee, h, depth - 1);
            }
        }
        Expression::NewExpression(n) => {
            n.arguments.len().hash(h);
            if depth > 0 {
                expression_key(&n.callee, h, depth - 1);
            }
        }
        Expression::AwaitExpression(a) if depth > 0 => expression_key(&a.argument, h, depth - 1),
        Expression::AssignmentExpression(a) => {
            match &a.left {
                AssignmentTarget::AssignmentTargetIdentifier(id) => id.name.as_str().hash(h),
                AssignmentTarget::StaticMemberExpression(m) => m.property.name.as_str().hash(h),
                _ => {}
            }
            if depth > 0 {
                expression_key(&a.right, h, depth - 1);
            }
        }
        _ => {}
    }
}

/// Multiplier of the polynomial hash.
const P: u64 = 0x0100_0000_01b3;

/// Hash of a body's source text without comments and without what the AST does not keep
/// (whitespace, optional semicolons and commas, parentheses), with quote styles folded. The AST
/// comparison has the final word, so collisions only cost time.
fn text_hash(ctx: &Ctx<'_>, span: Span) -> u64 {
    let src = ctx.source().as_bytes();
    let comments = ctx.comments();
    let end = (span.end as usize).min(src.len());
    let mut pos = span.start as usize;
    let mut ci = comments.partition_point(|c| (c.span.end as usize) <= pos);
    let mut hash = 0u64;
    while pos < end {
        if let Some(c) = comments.get(ci) {
            if (c.span.start as usize) <= pos {
                pos = (c.span.end as usize).max(pos + 1);
                ci += 1;
                continue;
            }
        }
        let Some(&b) = src.get(pos) else {
            break;
        };
        pos += 1;
        let b = match b {
            b'"' => b'\'',
            b'(' | b')' | b';' | b',' => continue,
            b if b.is_ascii_whitespace() => continue,
            b => b,
        };
        hash = hash.wrapping_mul(P).wrapping_add(u64::from(b) + 1);
    }
    hash
}
