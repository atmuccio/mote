//! Helpers shared by the batch-14 rules (batch 9's `util` covers call arguments, type names and
//! string constants). Nothing here walks the whole tree.

use crate::prelude::*;

/// What to do after visiting a node in [`walk`].
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
pub(super) fn walk<'t>(root: Node<'t>, mut f: impl FnMut(Node<'t>) -> Walk) -> bool {
    let mut cursor = root.walk();
    if !cursor.goto_first_child() {
        return false;
    }
    loop {
        let n = cursor.node();
        let action = if n.is_named() { f(n) } else { Walk::Skip };
        match action {
            Walk::Stop => return true,
            Walk::Continue if cursor.goto_first_child() => continue,
            _ => {}
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() || cursor.node().id() == root.id() {
                return false;
            }
        }
    }
}

/// Nodes whose code runs in another context: lambdas and class bodies (anonymous and local
/// classes).
pub(super) fn is_other_context(n: Node<'_>) -> bool {
    matches!(kind(n), "lambda_expression" | "class_body")
}
