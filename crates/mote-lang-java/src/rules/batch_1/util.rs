//! Helpers shared by the batch-1 rules. Nothing here walks the whole tree: every function looks
//! at one bounded subtree (an expression, a statement) or at the ancestors of a node.

use crate::prelude::*;
use std::sync::LazyLock;
use tree_sitter::TreeCursor;

/// Grammar fields used by this batch. `Node::child_by_field_name` resolves the name with a
/// linear scan of the grammar's field table on every call; these rules run on very common
/// nodes (every call, every binary expression), so the ids are resolved once.
#[derive(Clone, Copy)]
pub enum F {
    Left,
    Right,
    Name,
    Type,
    Body,
    Object,
    Alternative,
    Field,
    Operand,
    Consequence,
    Condition,
    Dimensions,
    Init,
    Array,
    Operator,
    Arguments,
    Value,
}

const FIELD_NAMES: [&str; 17] = [
    "left",
    "right",
    "name",
    "type",
    "body",
    "object",
    "alternative",
    "field",
    "operand",
    "consequence",
    "condition",
    "dimensions",
    "init",
    "array",
    "operator",
    "arguments",
    "value",
];

static FIELD_IDS: LazyLock<[u16; 17]> = LazyLock::new(|| {
    let lang = tree_sitter::Language::new(tree_sitter_java::LANGUAGE);
    FIELD_NAMES.map(|n| lang.field_id_for_name(n).map(|f| f.get()).unwrap_or(0))
});

/// Field access by cached id.
pub trait Fld<'t> {
    fn fld(&self, f: F) -> Option<Node<'t>>;
}

impl<'t> Fld<'t> for Node<'t> {
    #[inline]
    fn fld(&self, f: F) -> Option<Node<'t>> {
        let id = FIELD_IDS.get(f as usize).copied().unwrap_or(0);
        if id == 0 {
            return None;
        }
        self.child_by_field_id(id)
    }
}

/// The operator of a binary/unary/assignment expression as a string (`"=="`, `"+="`), or `""`.
#[inline]
pub fn op_str(expr: Node<'_>) -> &'static str {
    expr.fld(F::Operator).map(kind).unwrap_or("")
}

/// The operator token of an expression.
#[inline]
pub fn op(expr: Node<'_>) -> Option<Node<'_>> {
    expr.fld(F::Operator)
}

/// The argument expressions of a call or object creation.
pub fn args(call: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    call.fld(F::Arguments).into_iter().flat_map(named_children)
}

/// Number of arguments of a call.
pub fn arg_count(call: Node<'_>) -> usize {
    call.fld(F::Arguments)
        .map(|a| a.named_child_count())
        .map(|n| if n == 0 { 0 } else { args(call).count() })
        .unwrap_or(0)
}

/// The tokens (leaf nodes) of a subtree in source order, comments skipped. Iterative, so deep
/// expressions (long concatenation chains) cannot overflow the stack.
pub struct Tokens<'t> {
    cursor: TreeCursor<'t>,
    depth: u32,
    started: bool,
    done: bool,
}

impl<'t> Iterator for Tokens<'t> {
    type Item = Node<'t>;

    fn next(&mut self) -> Option<Node<'t>> {
        if self.done {
            return None;
        }
        if self.started && !self.advance() {
            return None;
        }
        self.started = true;
        loop {
            let n = self.cursor.node();
            if syntax::is_comment(n) {
                if !self.advance() {
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

impl Tokens<'_> {
    /// Move to the next sibling, climbing up as needed; `false` (and done) at the end.
    fn advance(&mut self) -> bool {
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

/// Iterate the tokens of `node` (comments excluded).
pub fn tokens(node: Node<'_>) -> Tokens<'_> {
    Tokens {
        cursor: node.walk(),
        depth: 0,
        started: false,
        done: false,
    }
}

/// Whether two expressions/statements are the same code, ignoring enclosing parentheses,
/// whitespace, and comments (the token sequences are compared; literal contents matter).
pub fn same_code(a: Node<'_>, b: Node<'_>, src: &str) -> bool {
    let a = syntax::strip_parens(a);
    let b = syntax::strip_parens(b);
    if a.id() == b.id() {
        return true;
    }
    let (ta, tb) = (syntax::node_text(a, src), syntax::node_text(b, src));
    if ta == tb {
        return true;
    }
    // Fast reject without touching the tree: texts that differ once whitespace is ignored can
    // only be the same code if a comment accounts for the difference.
    let same_without_ws = ta
        .bytes()
        .filter(|c| !c.is_ascii_whitespace())
        .eq(tb.bytes().filter(|c| !c.is_ascii_whitespace()));
    if !same_without_ws && !has_comment_marker(ta) && !has_comment_marker(tb) {
        return false;
    }
    let (mut ia, mut ib) = (tokens(a), tokens(b));
    loop {
        match (ia.next(), ib.next()) {
            (None, None) => return true,
            (Some(x), Some(y)) => {
                if syntax::node_text(x, src) != syntax::node_text(y, src) {
                    return false;
                }
            }
            _ => return false,
        }
    }
}

/// Whether the text may contain a comment (`//` or `/*`).
fn has_comment_marker(t: &str) -> bool {
    t.contains('/') && (t.contains("//") || t.contains("/*"))
}

/// Whether any node of the subtree (the node itself included) satisfies `pred`. Iterative,
/// bounded by the subtree.
pub fn any_in_subtree(node: Node<'_>, mut pred: impl FnMut(Node<'_>) -> bool) -> bool {
    let mut c = node.walk();
    let mut depth = 0u32;
    loop {
        if pred(c.node()) {
            return true;
        }
        if c.goto_first_child() {
            depth += 1;
            continue;
        }
        loop {
            if depth == 0 {
                return false;
            }
            if c.goto_next_sibling() {
                break;
            }
            if !c.goto_parent() {
                return false;
            }
            depth -= 1;
        }
    }
}

/// Whether evaluating the expression may have an effect or yield a fresh value each time:
/// calls, object/array creation, assignments, increments, lambdas, method references.
pub fn has_side_effects(node: Node<'_>) -> bool {
    any_in_subtree(node, |n| {
        matches!(
            kind(n),
            "method_invocation"
                | "object_creation_expression"
                | "array_creation_expression"
                | "assignment_expression"
                | "update_expression"
                | "lambda_expression"
                | "method_reference"
                | "switch_expression"
                | "ERROR"
        )
    })
}

/// Whether the expression writes something (assignment, `++`/`--`) or creates objects.
/// Unlike [`has_side_effects`], method calls are allowed.
pub fn has_mutation(node: Node<'_>) -> bool {
    any_in_subtree(node, |n| {
        matches!(
            kind(n),
            "assignment_expression"
                | "update_expression"
                | "object_creation_expression"
                | "array_creation_expression"
                | "ERROR"
        )
    })
}

/// Literal kinds (numbers, characters, strings, booleans, `null`).
pub fn is_literal(node: Node<'_>) -> bool {
    matches!(
        kind(syntax::strip_parens(node)),
        "decimal_integer_literal"
            | "hex_integer_literal"
            | "octal_integer_literal"
            | "binary_integer_literal"
            | "decimal_floating_point_literal"
            | "hex_floating_point_literal"
            | "character_literal"
            | "string_literal"
            | "true"
            | "false"
            | "null_literal"
    )
}

/// `float`/`double` or their wrappers.
pub fn is_floating(t: &JType) -> bool {
    let p = t.prim().or_else(|| t.unboxed());
    matches!(p, Some(Prim::Float | Prim::Double))
}

/// Primitive `float`/`double` only.
pub fn is_floating_primitive(t: &JType) -> bool {
    matches!(t.prim(), Some(Prim::Float | Prim::Double))
}

/// Loop statement kinds.
pub fn is_loop(node: Node<'_>) -> bool {
    matches!(
        kind(node),
        "for_statement" | "enhanced_for_statement" | "while_statement" | "do_statement"
    )
}

/// Function and type boundaries: an ancestor walk looking for statements of the "same code
/// body" stops here.
pub fn is_boundary(node: Node<'_>) -> bool {
    matches!(
        kind(node),
        "method_declaration"
            | "constructor_declaration"
            | "compact_constructor_declaration"
            | "lambda_expression"
            | "static_initializer"
            | "class_body"
            | "interface_body"
            | "enum_body"
            | "enum_body_declarations"
            | "annotation_type_body"
    )
}

/// The innermost loop whose **body** contains `node` (not its header), without crossing a
/// function or type boundary.
pub fn enclosing_loop_body<'t>(ctx: &Ctx<'t>, node: Node<'t>) -> Option<Node<'t>> {
    let mut prev = node;
    for a in ctx.ancestors(node) {
        if is_boundary(a) {
            return None;
        }
        if is_loop(a) && a.fld(F::Body).is_some_and(|b| b.id() == prev.id()) {
            return Some(a);
        }
        prev = a;
    }
    None
}

/// The node's parent after skipping enclosing parentheses, plus the child of that parent on the
/// path to `node` (the outermost parenthesized expression, or `node` itself).
pub fn parent_skipping_parens<'t>(ctx: &Ctx<'t>, node: Node<'t>) -> Option<(Node<'t>, Node<'t>)> {
    let mut child = node;
    for a in ctx.ancestors(node) {
        if kind(a) == "parenthesized_expression" {
            child = a;
            continue;
        }
        return Some((a, child));
    }
    None
}

/// Nesting depth above which rules skip resolver queries on the children of the node being
/// checked. Resolving a node that is not on the walk stack costs one descent from the root, so
/// querying every link of a pathological 50 000-term `a + b + …` chain would be quadratic.
const DEEP: usize = 256;

/// Whether the node being checked is nested more than [`DEEP`] levels (O(DEEP) to answer).
pub fn too_deep<'t>(ctx: &Ctx<'t>, node: Node<'t>) -> bool {
    ctx.ancestors(node).nth(DEEP).is_some()
}

/// The next named, non-comment sibling of a node.
pub fn next_statement<'t>(ctx: &Ctx<'t>, node: Node<'t>) -> Option<Node<'t>> {
    let mut n = ctx.next_named_sibling(node);
    while let Some(s) = n {
        if !syntax::is_comment(s) {
            return Some(s);
        }
        n = ctx.next_named_sibling(s);
    }
    None
}

/// The last named, non-comment child of a node.
pub fn last_named_child(node: Node<'_>) -> Option<Node<'_>> {
    named_children(node).last()
}

/// The first anonymous child token with the given text (`"else"`, `"if"`, `"try"`).
pub fn keyword<'t>(node: Node<'t>, kw: &str) -> Option<Node<'t>> {
    syntax::children(node).find(|c| !c.is_named() && kind(*c) == kw)
}

/// The simple method name of a `method_invocation`, or `""`.
pub fn method_name<'t>(ctx: &Ctx<'t>, call: Node<'t>) -> &'t str {
    call.fld(F::Name).map(|n| ctx.text(n)).unwrap_or("")
}

/// The explicit receiver of a call unless it is `this` or `super`.
pub fn plain_receiver(call: Node<'_>) -> Option<Node<'_>> {
    call.fld(F::Object)
        .filter(|o| !matches!(kind(*o), "this" | "super"))
}

/// Tri-state "can this statement complete normally?" (JLS 14.22, simplified):
/// `Some(false)` when it certainly jumps away (return/throw/break/continue/yield),
/// `Some(true)` when it certainly falls through to the next statement, `None` when unsure.
pub fn completes_normally<'t>(ctx: &Ctx<'t>, stmt: Node<'t>) -> Option<bool> {
    completes_normally_d(ctx, stmt, 0)
}

fn completes_normally_d<'t>(ctx: &Ctx<'t>, stmt: Node<'t>, depth: u32) -> Option<bool> {
    if depth > 64 {
        return None;
    }
    match kind(stmt) {
        "return_statement" | "throw_statement" | "break_statement" | "continue_statement"
        | "yield_statement" => Some(false),
        "block" => match last_named_child(stmt) {
            None => Some(true),
            Some(last) => completes_normally_d(ctx, last, depth + 1),
        },
        "if_statement" => {
            // Without `else` the `if` can always complete normally (condition false).
            let Some(alt) = stmt.fld(F::Alternative) else {
                return Some(true);
            };
            let then = stmt.fld(F::Consequence)?;
            let a = completes_normally_d(ctx, then, depth + 1);
            let b = completes_normally_d(ctx, alt, depth + 1);
            match (a, b) {
                (Some(false), Some(false)) => Some(false),
                (Some(true), _) | (_, Some(true)) => Some(true),
                _ => None,
            }
        }
        "synchronized_statement" => completes_normally_d(ctx, stmt.fld(F::Body)?, depth + 1),
        "try_statement" => {
            let body = completes_normally_d(ctx, stmt.fld(F::Body)?, depth + 1);
            let mut any_true = body == Some(true);
            let mut all_false = body == Some(false);
            let mut finally_jumps = false;
            for c in named_children(stmt) {
                match kind(c) {
                    "catch_clause" => {
                        let r = c
                            .fld(F::Body)
                            .and_then(|b| completes_normally_d(ctx, b, depth + 1));
                        any_true |= r == Some(true);
                        all_false &= r == Some(false);
                    }
                    "finally_clause" => {
                        let r = syntax::child_of_kind(c, "block")
                            .and_then(|b| completes_normally_d(ctx, b, depth + 1));
                        finally_jumps = r == Some(false);
                    }
                    _ => {}
                }
            }
            if finally_jumps || all_false {
                Some(false)
            } else if any_true {
                Some(true)
            } else {
                None
            }
        }
        // Loops may be infinite, switches and labels may hide jumps: don't guess.
        "while_statement"
        | "do_statement"
        | "for_statement"
        | "enhanced_for_statement"
        | "switch_expression"
        | "labeled_statement"
        | "try_with_resources_statement" => None,
        "expression_statement" => {
            // Helpers that never return normally (`System.exit`, `fail(..)`, `throwIfX(..)`).
            let e = syntax::first_named_child(stmt)?;
            if kind(e) == "method_invocation" {
                let name = method_name(ctx, e);
                let lower_starts = |p: &str| {
                    name.get(..p.len())
                        .is_some_and(|s| s.eq_ignore_ascii_case(p))
                };
                if matches!(name, "exit" | "halt")
                    || lower_starts("throw")
                    || lower_starts("fail")
                    || lower_starts("abort")
                    || lower_starts("rethrow")
                {
                    return None;
                }
            }
            Some(true)
        }
        "ERROR" => None,
        _ => Some(true),
    }
}

/// Whether a loop does its work in its header, so an empty body is intended: a `while`/`do`
/// whose condition calls a method, or a `for` whose condition calls a method or whose counter
/// is not declared by the loop itself (a search loop, `for (;;)`).
pub fn loop_header_does_work(loop_: Node<'_>) -> bool {
    let cond_effects = loop_.fld(F::Condition).is_some_and(has_side_effects);
    match kind(loop_) {
        "while_statement" | "do_statement" => cond_effects,
        "for_statement" => {
            cond_effects
                || loop_
                    .fld(F::Init)
                    .is_none_or(|i| kind(i) != "local_variable_declaration")
        }
        _ => false,
    }
}
