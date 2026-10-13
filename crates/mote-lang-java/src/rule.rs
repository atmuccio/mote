//! The rule-author API: the [`JavaRule`] trait and [`Span`].
//!
//! See `RULE_AUTHORING.md` at the crate root for the guide.

use crate::ctx::Ctx;
use mote_core::RuleMeta;
use tree_sitter::Node;

/// A Java rule.
///
/// A **fresh instance is created for every analyzed file** (via `rules::all()`), so a rule may
/// keep per-file state in its own fields — including `Node<'t>` and `&'t str` borrowed from the
/// file — and report it in [`finish`](JavaRule::finish). Constructors must be trivial (no
/// allocation, no tables): put constant tables in `static`s.
///
/// The plugin walks each syntax tree **once** and calls [`check`](JavaRule::check) for every
/// node whose kind is listed in [`kinds`](JavaRule::kinds), in source (pre-)order. Rules must not
/// walk the whole tree themselves; looking at a node's children, a bounded subtree, or its
/// ancestors is fine.
pub trait JavaRule<'t> {
    /// Static metadata; `meta().id` is the rule id (`java/<kebab-name>`).
    fn meta(&self) -> &'static RuleMeta;

    /// Named node kinds of the Java grammar this rule wants to see (e.g. `&["catch_clause"]`).
    /// An empty slice is fine for rules that only use [`finish`](JavaRule::finish).
    fn kinds(&self) -> &'static [&'static str];

    /// For rules subscribed to `method_invocation`: the method names the rule is about. When
    /// non-empty, `check` only sees the `method_invocation` nodes whose name
    /// ([`Ctx::call_name`]) is in the list, plus those without a name (syntax errors); other
    /// subscribed kinds are unaffected. Declare it when `check` does nothing for any other name:
    /// the walker then skips the call altogether, which matters on a node this common.
    fn call_names(&self) -> &'static [&'static str] {
        &[]
    }

    /// For rules subscribed to `method_declaration`: the declared method names the rule is
    /// about, like [`call_names`](JavaRule::call_names) for calls. When non-empty, `check`
    /// only sees the `method_declaration` nodes whose name (the `name` field) is in the list,
    /// plus those without a name; other subscribed kinds are unaffected. Declare it when
    /// `check` does nothing for any other method (`equals`, `finalize`, `compareTo`, …).
    fn method_names(&self) -> &'static [&'static str] {
        &[]
    }

    /// Called for each node of a subscribed kind, parents before children.
    fn check(&mut self, _node: Node<'t>, _ctx: &Ctx<'t>) {}

    /// Called once per file after the traversal (every comment has been collected by then).
    fn finish(&mut self, _ctx: &Ctx<'t>) {}
}

/// A boxed rule instance for one file.
pub type BoxedRule<'t> = Box<dyn JavaRule<'t> + 't>;

/// A byte range `[start, end)` in the source file. `Copy`, so it can be stored anywhere; build
/// one from a node (`Span::from(node)`), a pair of nodes (first start to last end), or a
/// byte range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Span {
            start: start as u32,
            end: end.max(start) as u32,
        }
    }
    /// From the start of `first` to the end of `last`.
    pub fn cover(first: Node<'_>, last: Node<'_>) -> Self {
        Span::new(first.start_byte(), last.end_byte().max(first.start_byte()))
    }
    pub fn is_empty(&self) -> bool {
        self.end <= self.start
    }
}

impl From<Node<'_>> for Span {
    fn from(n: Node<'_>) -> Self {
        Span::new(n.start_byte(), n.end_byte())
    }
}

impl From<(Node<'_>, Node<'_>)> for Span {
    fn from((a, b): (Node<'_>, Node<'_>)) -> Self {
        Span::cover(a, b)
    }
}

impl From<std::ops::Range<usize>> for Span {
    fn from(r: std::ops::Range<usize>) -> Self {
        Span::new(r.start, r.end)
    }
}
