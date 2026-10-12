//! [`DeclaredTypes`]: a per-file syntactic index `variable name → declared type node`, filled
//! by the walker (see [`crate::Ctx::declared_types`]).

use crate::kinds::{k, kind, K};
use crate::syntax::{self, named_children, node_text, Field, NodeExt};
use rustc_hash::FxHashMap;
use tree_sitter::Node;

/// Every variable, field, parameter, resource, and `for` variable of the file with the type
/// node it was declared with — purely syntactic: one hash insert per declared name, no
/// resolver. Rules use it to pre-qualify the few nodes worth resolving (`x.contains(y)` only
/// matters when `x` was declared as a collection).
///
/// The walker records a declaration when it enters it, so during `check` the index holds the
/// declarations that precede the current node (and the current node, if it is one); it is
/// complete in `finish`. A name declared with different type texts (in different methods)
/// keeps each of them.
#[derive(Default)]
pub struct DeclaredTypes<'t> {
    by_name: FxHashMap<&'t str, Node<'t>>,
    /// Further declarations of a name with a different type text.
    extra: Vec<(&'t str, Node<'t>)>,
}

impl<'t> DeclaredTypes<'t> {
    /// Whether the walker records nodes of this kind: `local_variable_declaration`,
    /// `field_declaration`, `constant_declaration`, `formal_parameter`, `spread_parameter`,
    /// `resource`, `enhanced_for_statement`.
    pub(crate) fn is_declaration(kk: K) -> bool {
        matches!(
            kk,
            K::LocalVarDecl
                | K::FieldDecl
                | K::ConstantDecl
                | K::FormalParameter
                | K::SpreadParameter
                | K::Resource
                | K::EnhancedForStatement
        )
    }

    /// Record the names declared by `node` (a kind accepted by [`Self::is_declaration`]).
    pub(crate) fn collect(&mut self, node: Node<'t>, src: &'t str) {
        let kk = k(node);
        let ty = match kk {
            K::SpreadParameter => named_children(node)
                .find(|c| !matches!(kind(*c), "variable_declarator" | "modifiers")),
            _ => node.field(Field::Type),
        };
        let Some(ty) = ty else {
            return;
        };
        let ty_text = node_text(ty, src);
        let mut add = |name: Option<Node<'t>>| {
            let Some(n) = name else { return };
            let name = node_text(n, src);
            match self.by_name.entry(name) {
                std::collections::hash_map::Entry::Occupied(e) => {
                    if node_text(*e.get(), src) != ty_text {
                        self.extra.push((name, ty));
                    }
                }
                std::collections::hash_map::Entry::Vacant(v) => {
                    v.insert(ty);
                }
            }
        };
        match kk {
            K::LocalVarDecl | K::FieldDecl | K::ConstantDecl => {
                for d in syntax::declarators(node) {
                    add(d.field(Field::Name));
                }
            }
            K::SpreadParameter => add(named_children(node)
                .find(|c| kind(*c) == "variable_declarator")
                .and_then(|d| d.field(Field::Name))),
            _ => add(node.field(Field::Name)),
        }
    }

    /// The declared type node of `name`, when every declaration of that name in the file has
    /// the same type text.
    pub fn type_of_name(&self, name: &str) -> Option<Node<'t>> {
        if self.extra.iter().any(|(n, _)| *n == name) {
            return None;
        }
        self.by_name.get(name).copied()
    }

    /// Every declared type node of `name` (one per distinct type text).
    pub fn types_of<'a>(&'a self, name: &'a str) -> impl Iterator<Item = Node<'t>> + 'a {
        self.by_name.get(name).copied().into_iter().chain(
            self.extra
                .iter()
                .filter(move |(n, _)| *n == name)
                .map(|(_, t)| *t),
        )
    }
}
