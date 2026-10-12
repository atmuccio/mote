//! Private helpers shared by the batch 5 rules.

use crate::prelude::*;
use oxc_span::ContentEq;

/// The nodes below `root`, in source (pre-)order. The semantic builder numbers nodes in traversal
/// order, so a subtree is the run of ids that follows its root and stays inside the root's span.
pub(super) fn descendants<'a>(
    ctx: &Ctx<'a>,
    root: &AstNode<'a>,
) -> impl Iterator<Item = &'a AstNode<'a>> + 'a {
    let nodes = ctx.nodes();
    let span = root.span();
    let start = root.id().index() + 1;
    (start..nodes.len())
        .map(move |i| nodes.get_node(NodeId::new(i)))
        .take_while(move |n| {
            let s = n.span();
            s.start < span.end && s.end <= span.end
        })
}

/// The function (not arrow) whose `this` a node sees, or `None` at the top level. Class bodies
/// (field initializers, static blocks) have their own `this`, so a class also ends the search and
/// is returned as the owner.
pub(super) fn this_owner<'a>(ctx: &Ctx<'a>, node: &AstNode<'a>) -> Option<&'a AstNode<'a>> {
    ctx.ancestors(node)
        .find(|a| matches!(a.kind(), AstKind::Function(_) | AstKind::Class(_)))
}

/// The nearest function, arrow function, or class enclosing a node: code inside it does not run
/// when the surrounding statements run, but later (when called or instantiated).
pub(super) fn deferred_owner<'a>(ctx: &Ctx<'a>, node: &AstNode<'a>) -> Option<NodeId> {
    ctx.ancestors(node)
        .find(|a| {
            matches!(
                a.kind(),
                AstKind::Function(_)
                    | AstKind::ArrowFunctionExpression(_)
                    | AstKind::Class(_)
                    | AstKind::TSModuleDeclaration(_)
            )
        })
        .map(AstNode::id)
}

/// Is `expr` free of side effects, so that two occurrences of it are guaranteed to evaluate the
/// same way? Identifiers, `this`, primitive literals, member accesses, and operators over those.
pub(super) fn is_pure(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::Identifier(_)
        | Expression::ThisExpression(_)
        | Expression::StringLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BigIntLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_) => true,
        Expression::TemplateLiteral(t) => t.expressions.iter().all(is_pure),
        Expression::StaticMemberExpression(m) => is_pure(&m.object),
        Expression::PrivateFieldExpression(m) => is_pure(&m.object),
        Expression::ComputedMemberExpression(m) => is_pure(&m.object) && is_pure(&m.expression),
        Expression::ChainExpression(c) => match &c.expression {
            ChainElement::StaticMemberExpression(m) => is_pure(&m.object),
            ChainElement::PrivateFieldExpression(m) => is_pure(&m.object),
            ChainElement::ComputedMemberExpression(m) => {
                is_pure(&m.object) && is_pure(&m.expression)
            }
            ChainElement::TSNonNullExpression(e) => is_pure(&e.expression),
            ChainElement::CallExpression(_) => false,
        },
        Expression::UnaryExpression(u) => {
            u.operator != UnaryOperator::Delete && is_pure(&u.argument)
        }
        Expression::BinaryExpression(b) => is_pure(&b.left) && is_pure(&b.right),
        Expression::LogicalExpression(l) => is_pure(&l.left) && is_pure(&l.right),
        Expression::ConditionalExpression(c) => {
            is_pure(&c.test) && is_pure(&c.consequent) && is_pure(&c.alternate)
        }
        Expression::TSAsExpression(e) => is_pure(&e.expression),
        Expression::TSSatisfiesExpression(e) => is_pure(&e.expression),
        Expression::TSNonNullExpression(e) => is_pure(&e.expression),
        Expression::TSTypeAssertion(e) => is_pure(&e.expression),
        _ => false,
    }
}

/// Two side-effect-free expressions with the same structure (spans and formatting ignored).
pub(super) fn same_pure_expr(a: &Expression<'_>, b: &Expression<'_>) -> bool {
    a.content_eq(b) && is_pure(a)
}

/// Does the expression read a variable, `this`, or a property (as opposed to only literals)?
pub(super) fn mentions_variable(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::Identifier(_)
        | Expression::ThisExpression(_)
        | Expression::StaticMemberExpression(_)
        | Expression::ComputedMemberExpression(_)
        | Expression::PrivateFieldExpression(_)
        | Expression::ChainExpression(_) => true,
        Expression::TemplateLiteral(t) => t.expressions.iter().any(mentions_variable),
        Expression::UnaryExpression(u) => mentions_variable(&u.argument),
        Expression::BinaryExpression(b) => {
            mentions_variable(&b.left) || mentions_variable(&b.right)
        }
        Expression::LogicalExpression(l) => {
            mentions_variable(&l.left) || mentions_variable(&l.right)
        }
        Expression::ConditionalExpression(c) => {
            mentions_variable(&c.test)
                || mentions_variable(&c.consequent)
                || mentions_variable(&c.alternate)
        }
        Expression::TSAsExpression(e) => mentions_variable(&e.expression),
        Expression::TSSatisfiesExpression(e) => mentions_variable(&e.expression),
        Expression::TSNonNullExpression(e) => mentions_variable(&e.expression),
        Expression::TSTypeAssertion(e) => mentions_variable(&e.expression),
        _ => false,
    }
}

/// A short, single-line preview of source text for messages (at most 40 characters).
pub(super) fn excerpt(text: &str) -> String {
    let line = text.lines().next().unwrap_or("");
    let mut out: String = line
        .chars()
        .take(40)
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    if out.len() < text.len() {
        out.push('…');
    }
    out
}

/// Span of the first occurrence of `needle` in the source between `from` and `to`, if any.
pub(super) fn find_in(ctx: &Ctx<'_>, from: u32, to: u32, needle: &str) -> Option<Span> {
    let text = ctx.text(Span::new(from, to));
    let pos = text.find(needle)? as u32;
    Some(Span::new(from + pos, from + pos + needle.len() as u32))
}

/// Does the file keep its top-level bindings private: an ES module, a CommonJS file, or a script
/// that uses `require` / `module` / `exports`? In a classic script, top-level declarations are
/// globals shared with every other script.
pub(super) fn has_private_top_level(ctx: &Ctx<'_>) -> bool {
    ctx.is_module()
        || ctx.source_type().is_commonjs()
        || ctx
            .scoping()
            .root_unresolved_references()
            .contains_key("require")
        || ctx
            .scoping()
            .root_unresolved_references()
            .contains_key("module")
        || ctx
            .scoping()
            .root_unresolved_references()
            .contains_key("exports")
}

/// How a local collection (`[]`, `new Map()`, `new Set()`) is used across the file.
#[derive(Default)]
pub(super) struct CollectionUse {
    /// Calls that add, remove, or reorder elements, and element writes (`a[i] = v`).
    pub mutations: usize,
    /// Mutations whose result is discarded (a statement of its own).
    pub discarded_mutations: usize,
    /// Reads of the contents: iteration, lookups, `.length`, element reads.
    pub reads: Vec<Span>,
    /// Spreads of the contents into an array or argument list (`[...a, ...b]`).
    pub spreads: usize,
    /// Any other use (passed to a function, returned, stored elsewhere, reassigned, an unknown
    /// method): the collection escapes local reasoning.
    pub escapes: bool,
}

/// The local collection declared by `declarator`, when it is a `const`/`let` initialized with an
/// array literal or `new Map/Set/WeakMap/WeakSet/Array(…)`, not exported and not a global of a
/// classic script. Returns the symbol and whether the initial collection is empty.
pub(super) fn local_collection(
    ctx: &Ctx<'_>,
    node: &AstNode<'_>,
    declarator: &VariableDeclarator<'_>,
) -> Option<(SymbolId, bool)> {
    let init = helpers::unwrap_expr(declarator.init.as_ref()?);
    let empty = match init {
        Expression::ArrayExpression(a) => a.elements.is_empty(),
        Expression::NewExpression(new) => {
            let Expression::Identifier(callee) = &new.callee else {
                return None;
            };
            if !matches!(
                callee.name.as_str(),
                "Map" | "Set" | "WeakMap" | "WeakSet" | "Array"
            ) || !ctx.is_global_reference(callee)
            {
                return None;
            }
            new.arguments.is_empty()
        }
        _ => return None,
    };
    if !matches!(
        declarator.kind,
        VariableDeclarationKind::Const | VariableDeclarationKind::Let
    ) {
        return None;
    }
    let BindingPattern::BindingIdentifier(id) = &declarator.id else {
        return None;
    };
    let symbol = ctx.binding_symbol(id)?;
    // `export const list = []`: other modules may read it. (`export { list }` and
    // `export default list` are references, which `collection_use` treats as escapes.)
    let exported = ctx.ancestors(node).take(3).any(|a| {
        matches!(
            a.kind(),
            AstKind::ExportNamedDeclaration(_) | AstKind::ExportDefaultDeclaration(_)
        )
    });
    if exported || ctx.in_ambient_context(node) {
        return None;
    }
    let top_level = ctx.scoping().symbol_scope_id(symbol) == ctx.scoping().root_scope_id();
    if top_level && !has_private_top_level(ctx) {
        return None;
    }
    Some((symbol, empty))
}

const MUTATORS: &[&str] = &[
    "push",
    "unshift",
    "add",
    "set",
    "delete",
    "clear",
    "splice",
    "fill",
    "sort",
    "reverse",
    "copyWithin",
];

const READERS: &[&str] = &[
    "forEach",
    "map",
    "filter",
    "find",
    "findIndex",
    "findLast",
    "findLastIndex",
    "some",
    "every",
    "includes",
    "indexOf",
    "lastIndexOf",
    "join",
    "reduce",
    "reduceRight",
    "slice",
    "at",
    "get",
    "has",
    "keys",
    "values",
    "entries",
    "concat",
    "flat",
    "flatMap",
    "pop",
    "shift",
    "toSorted",
    "toReversed",
    "toSpliced",
    "with",
];

/// Classify every use of the collection bound to `symbol`.
pub(super) fn collection_use(ctx: &Ctx<'_>, symbol: SymbolId) -> CollectionUse {
    let mut out = CollectionUse::default();
    for reference in ctx.references_of(symbol) {
        if reference.flags().is_type_only() {
            continue;
        }
        if reference.is_write() {
            out.escapes = true;
            return out;
        }
        let node = ctx.node(reference.node_id());
        let span = node.span();
        let Some(parent) = ctx.parent(node) else {
            out.escapes = true;
            return out;
        };
        match parent.kind() {
            AstKind::StaticMemberExpression(m) if m.object.span() == span => {
                let name = m.property.name.as_str();
                match ctx.parent_kind(parent) {
                    Some(AstKind::CallExpression(call)) if call.callee.span() == m.span => {
                        let call_node = ctx.parent(parent);
                        if MUTATORS.contains(&name) {
                            out.mutations += 1;
                            let discarded = call_node
                                .and_then(|c| ctx.outer_parent(c))
                                .is_some_and(|p| {
                                    matches!(p.kind(), AstKind::ExpressionStatement(_))
                                });
                            if discarded {
                                out.discarded_mutations += 1;
                            } else {
                                out.escapes = true;
                            }
                        } else if READERS.contains(&name) {
                            out.reads.push(span);
                        } else {
                            out.escapes = true;
                        }
                    }
                    Some(AstKind::AssignmentExpression(a)) if a.left.span() == m.span => {
                        if name == "length" {
                            out.mutations += 1;
                            out.discarded_mutations += 1;
                        } else {
                            out.escapes = true;
                        }
                    }
                    _ => {
                        if matches!(name, "length" | "size")
                            && !reference.flags().is_member_write_target()
                        {
                            out.reads.push(span);
                        } else {
                            out.escapes = true;
                        }
                    }
                }
            }
            AstKind::ComputedMemberExpression(m) if m.object.span() == span => {
                match ctx.parent_kind(parent) {
                    Some(AstKind::AssignmentExpression(a))
                        if a.left.span() == m.span && a.operator == AssignmentOperator::Assign =>
                    {
                        out.mutations += 1;
                        let discarded = ctx
                            .outer_parent(ctx.parent(parent).map_or(parent.id(), AstNode::id))
                            .is_some_and(|p| matches!(p.kind(), AstKind::ExpressionStatement(_)));
                        if discarded {
                            out.discarded_mutations += 1;
                        } else {
                            out.escapes = true;
                        }
                    }
                    _ if reference.flags().is_member_write_target() => out.escapes = true,
                    _ => out.reads.push(span),
                }
            }
            AstKind::ForOfStatement(f) if f.right.span() == span => out.reads.push(span),
            AstKind::ForInStatement(f) if f.right.span() == span => out.reads.push(span),
            AstKind::SpreadElement(_) => out.spreads += 1,
            _ => {
                out.escapes = true;
                return out;
            }
        }
    }
    out
}

/// Every resolved reference of the file that writes its variable (`x = …`, `x++`, `x += …`) or,
/// with `member_writes`, writes a property of it (`x.p = …`), with its symbol. One sequential scan
/// of the reference table: far cheaper than walking the references of every symbol.
pub(super) fn written_references<'a>(
    ctx: &Ctx<'a>,
    member_writes: bool,
) -> impl Iterator<Item = (&'a Reference, SymbolId)> + 'a {
    let scoping = ctx.scoping();
    (0..scoping.references_len()).filter_map(move |i| {
        let reference = scoping.get_reference(oxc_semantic::ReferenceId::new(i));
        let flags = reference.flags();
        let written = flags.is_write() || (member_writes && flags.is_member_write_target());
        if !written {
            return None;
        }
        reference.symbol_id().map(|symbol| (reference, symbol))
    })
}
