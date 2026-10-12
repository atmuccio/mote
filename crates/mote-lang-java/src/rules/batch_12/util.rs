//! Helpers shared by the batch-12 rules: bounded subtree visits, token-wise code comparison
//! and small syntax lookups. Nothing here walks the whole tree.

use crate::prelude::*;
use tree_sitter::TreeCursor;

/// What to do after visiting a node in [`visit`].
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Walk {
    /// Descend into the node's children.
    Continue,
    /// Don't look inside this node.
    Skip,
    /// End the whole visit.
    Stop,
}

/// Pre-order visit of the **named** descendants of `root` (not `root` itself), in source order.
/// Returns `true` when the callback stopped the visit. Linear in the size of the subtree.
pub(super) fn visit<'t>(root: Node<'t>, mut f: impl FnMut(Node<'t>) -> Walk) -> bool {
    let mut cursor = root.walk();
    if !cursor.goto_first_child() {
        return false;
    }
    let mut depth = 1u32;
    loop {
        let n = cursor.node();
        let action = if n.is_named() { f(n) } else { Walk::Skip };
        match action {
            Walk::Stop => return true,
            Walk::Continue if cursor.goto_first_child() => {
                depth += 1;
                continue;
            }
            _ => {}
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if depth <= 1 || !cursor.goto_parent() {
                return false;
            }
            depth -= 1;
        }
    }
}

/// Whether any named descendant of `root` (not `root` itself) satisfies `pred`.
pub(super) fn any_descendant<'t>(root: Node<'t>, mut pred: impl FnMut(Node<'t>) -> bool) -> bool {
    visit(root, |n| if pred(n) { Walk::Stop } else { Walk::Continue })
}

/// The leaves of a subtree in source order, comments excluded.
struct Leaves<'t> {
    cursor: TreeCursor<'t>,
    depth: u32,
    started: bool,
    done: bool,
}

impl<'t> Leaves<'t> {
    fn new(node: Node<'t>) -> Self {
        Leaves {
            cursor: node.walk(),
            depth: 0,
            started: false,
            done: false,
        }
    }

    /// Step to the next sibling, climbing as needed; `false` once the subtree is exhausted.
    fn step(&mut self) -> bool {
        loop {
            if self.depth == 0 {
                self.done = true;
                return false;
            }
            if self.cursor.goto_next_sibling() {
                return true;
            }
            if !self.cursor.goto_parent() {
                self.done = true;
                return false;
            }
            self.depth -= 1;
        }
    }
}

impl<'t> Iterator for Leaves<'t> {
    type Item = Node<'t>;

    fn next(&mut self) -> Option<Node<'t>> {
        if self.done || (self.started && !self.step()) {
            return None;
        }
        self.started = true;
        loop {
            let n = self.cursor.node();
            if syntax::is_comment(n) {
                if !self.step() {
                    return None;
                }
                continue;
            }
            if self.cursor.goto_first_child() {
                self.depth += 1;
                continue;
            }
            return Some(n);
        }
    }
}

/// Whether two subtrees are the same code: equal token sequences, whitespace and comments
/// ignored (literal contents matter).
pub(super) fn same_code(a: Node<'_>, b: Node<'_>, src: &str) -> bool {
    if a.id() == b.id() {
        return true;
    }
    let (ta, tb) = (syntax::node_text(a, src), syntax::node_text(b, src));
    if ta == tb {
        return true;
    }
    let squeezed_equal = ta
        .bytes()
        .filter(|c| !c.is_ascii_whitespace())
        .eq(tb.bytes().filter(|c| !c.is_ascii_whitespace()));
    let may_have_comment = |t: &str| t.contains("//") || t.contains("/*");
    if !squeezed_equal && !may_have_comment(ta) && !may_have_comment(tb) {
        return false;
    }
    let (mut la, mut lb) = (Leaves::new(a), Leaves::new(b));
    loop {
        match (la.next(), lb.next()) {
            (None, None) => return true,
            (Some(x), Some(y)) if syntax::node_text(x, src) == syntax::node_text(y, src) => {}
            _ => return false,
        }
    }
}

/// The first anonymous child token of `node` whose kind is `kw` (`if`, `else`, `catch`, …).
pub(super) fn keyword<'t>(node: Node<'t>, kw: &str) -> Option<Node<'t>> {
    syntax::children(node).find(|c| !c.is_named() && c.kind() == kw)
}

/// Whether `ann` (`annotation` / `marker_annotation`) is one of `fqns`, written fully
/// qualified or imported by a single-type or on-demand import.
pub(super) fn annotation_is(ctx: &Ctx<'_>, ann: Node<'_>, fqns: &[&str]) -> bool {
    let Some(name) = ann.field(Field::Name) else {
        return false;
    };
    let text = ctx.text(name);
    if text.contains('.') {
        return fqns.contains(&text);
    }
    let imports = ctx.imports();
    if let Some(imp) = imports
        .iter()
        .find(|i| !i.is_static && !i.is_wildcard && i.simple_name() == text)
    {
        return fqns.contains(&imp.path);
    }
    imports
        .iter()
        .filter(|i| !i.is_static && i.is_wildcard)
        .any(|imp| {
            fqns.iter().any(|f| {
                f.strip_suffix(text)
                    .and_then(|q| q.strip_suffix('.'))
                    .is_some_and(|q| q == imp.path)
            })
        })
}

/// The first annotation of `decl` that is one of `fqns`.
pub(super) fn find_annotation<'t>(
    ctx: &Ctx<'t>,
    decl: Node<'t>,
    fqns: &[&str],
) -> Option<Node<'t>> {
    syntax::annotations(decl).find(|a| {
        let simple = syntax::annotation_name(*a, ctx.source());
        let simple = simple.rsplit('.').next().unwrap_or(simple);
        fqns.iter().any(|f| f.rsplit('.').next() == Some(simple)) && annotation_is(ctx, *a, fqns)
    })
}

/// Per-file "does the source contain `needle`?", computed once.
#[derive(Default)]
pub(super) struct Gate(Option<bool>);

impl Gate {
    pub(super) fn open(&mut self, ctx: &Ctx<'_>, needle: &str) -> bool {
        *self.0.get_or_insert_with(|| ctx.source().contains(needle))
    }
}

/// Per-file "does the file import something under `prefix`?", computed once from the parsed
/// imports (cheaper than searching the whole source text).
#[derive(Default)]
pub(super) struct ImportGate(Option<bool>);

impl ImportGate {
    pub(super) fn open(&mut self, ctx: &Ctx<'_>, prefix: &str) -> bool {
        *self
            .0
            .get_or_insert_with(|| ctx.imports().iter().any(|i| i.path.starts_with(prefix)))
    }
}
