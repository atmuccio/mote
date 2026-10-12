//! Helpers shared by the batch-3 rules (bounded subtree visits, catch clauses, type names,
//! JUnit annotations, field lookup).

use crate::prelude::*;

/// Field ids of tree-sitter-java, resolved once: `child_by_field_id` avoids the per-call
/// field-name lookup of `child_by_field_name` on hot paths (every method call, declaration…).
pub(super) struct Fields {
    pub name: u16,
    pub object: u16,
    pub arguments: u16,
    pub value: u16,
}

pub(super) static FIELDS: std::sync::LazyLock<Fields> = std::sync::LazyLock::new(|| {
    let lang = tree_sitter::Language::new(tree_sitter_java::LANGUAGE);
    let id = |n: &str| lang.field_id_for_name(n).map_or(u16::MAX, |f| f.get());
    Fields {
        name: id("name"),
        object: id("object"),
        arguments: id("arguments"),
        value: id("value"),
    }
});

/// The `name` child of a node (method invocation, declaration, …).
#[inline]
pub(super) fn name_field(node: Node<'_>) -> Option<Node<'_>> {
    node.child_by_field_id(FIELDS.name)
}

/// The `object` (receiver) child of a method invocation or field access.
#[inline]
pub(super) fn object_field(node: Node<'_>) -> Option<Node<'_>> {
    node.child_by_field_id(FIELDS.object)
}

/// Number of arguments of a call (cached field id).
pub(super) fn arg_count(call: Node<'_>) -> usize {
    call.child_by_field_id(FIELDS.arguments)
        .map_or(0, |a| named_children(a).count())
}

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

/// Whether any named descendant of `root` satisfies `pred`; subtrees for which `skip` returns
/// `true` are not entered (and not tested).
pub(super) fn any_descendant<'t>(
    root: Node<'t>,
    mut skip: impl FnMut(Node<'t>) -> bool,
    mut pred: impl FnMut(Node<'t>) -> bool,
) -> bool {
    visit(root, |n| {
        if skip(n) {
            Walk::Skip
        } else if pred(n) {
            Walk::Stop
        } else {
            Walk::Continue
        }
    })
}

/// Nodes whose code runs in another context: lambdas and class bodies (anonymous and local
/// classes).
pub(super) fn is_other_context(n: Node<'_>) -> bool {
    matches!(kind(n), "lambda_expression" | "class_body")
}

/// The statements of a block (comments skipped).
pub(super) fn statements(block: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    named_children(block)
}

/// The `catch_formal_parameter` of a `catch_clause`.
pub(super) fn catch_param(catch_clause: Node<'_>) -> Option<Node<'_>> {
    named_children(catch_clause).find(|c| kind(*c) == "catch_formal_parameter")
}

/// The `catch_type` node of a `catch_clause`.
pub(super) fn catch_type(catch_clause: Node<'_>) -> Option<Node<'_>> {
    catch_param(catch_clause).and_then(|p| named_children(p).find(|c| kind(*c) == "catch_type"))
}

/// The name of the caught exception variable (`None` for `_`).
pub(super) fn catch_param_name<'t>(catch_clause: Node<'t>, ctx: &Ctx<'t>) -> Option<&'t str> {
    let name = catch_param(catch_clause)?.field(Field::Name)?;
    (kind(name) == "identifier").then(|| ctx.text(name))
}

/// Whether the last statement of `block` is a `throw` (the caught exception keeps propagating,
/// possibly wrapped).
pub(super) fn block_ends_with_throw(block: Node<'_>) -> bool {
    statements(block)
        .last()
        .is_some_and(|s| kind(s) == "throw_statement")
}

/// The name part of a type node: `Foo` for `Foo`, `a.b.Foo`, `Foo<T>`, `a.b.Foo<T>`.
pub(super) fn type_simple_name<'t>(type_node: Node<'t>, ctx: &Ctx<'t>) -> &'t str {
    let base = type_base(type_node);
    let text = ctx.text(base);
    text.rsplit('.').next().unwrap_or(text).trim()
}

/// The type node without type arguments (`Foo` of `Foo<T>`).
pub(super) fn type_base(type_node: Node<'_>) -> Node<'_> {
    if kind(type_node) == "generic_type" {
        syntax::first_named_child(type_node).unwrap_or(type_node)
    } else {
        type_node
    }
}

/// Fully-qualified name of a class type node as seen from its position (imports, this file,
/// `java.lang`, and imported library types). `None` when unresolvable.
pub(super) fn type_fqn<'t>(ctx: &Ctx<'t>, type_node: Node<'t>) -> Option<String> {
    let base = type_base(type_node);
    if !matches!(kind(base), "type_identifier" | "scoped_type_identifier") {
        return None;
    }
    let text = ctx.text(base);
    if text.contains(|c: char| c.is_whitespace() || c == '/') {
        return None;
    }
    ctx.resolve_type_name(text, base)
}

/// Whether the class type node names `fqn`. Besides [`type_fqn`], accepts a simple name that
/// only an on-demand import of `fqn`'s package can supply (for JDK classes missing from the
/// resolver's table, e.g. `java.io.RandomAccessFile` with `import java.io.*;`).
pub(super) fn type_is<'t>(ctx: &Ctx<'t>, type_node: Node<'t>, fqn: &str) -> bool {
    if let Some(f) = type_fqn(ctx, type_node) {
        return f == fqn;
    }
    let base = type_base(type_node);
    if kind(base) != "type_identifier" {
        return false;
    }
    let simple = ctx.text(base);
    let Some(pkg) = fqn.strip_suffix(simple).and_then(|p| p.strip_suffix('.')) else {
        return false;
    };
    ctx.imports()
        .iter()
        .any(|i| !i.is_static && i.is_wildcard && i.path == pkg)
}

/// Whether `ann` (`annotation` / `marker_annotation`) is one of `fqns` (e.g. `org.junit.Test`),
/// through a single-type import, an on-demand import, or a fully-qualified name.
pub(super) fn annotation_is(ctx: &Ctx<'_>, ann: Node<'_>, fqns: &[&str]) -> bool {
    let Some(name) = ann.field(Field::Name) else {
        return false;
    };
    let text = ctx.text(name);
    if text.contains('.') {
        return fqns.contains(&text);
    }
    // A single-type import decides; otherwise an on-demand import of the right package.
    if let Some(imp) = ctx
        .imports()
        .iter()
        .find(|i| !i.is_static && !i.is_wildcard && i.simple_name() == text)
    {
        return fqns.contains(&imp.path);
    }
    ctx.imports()
        .iter()
        .filter(|i| !i.is_static && i.is_wildcard)
        .any(|imp| {
            fqns.iter()
                .any(|f| f.strip_suffix(text).and_then(|q| q.strip_suffix('.')) == Some(imp.path))
        })
}

/// JUnit 4, JUnit 5, and TestNG `@Test` annotations.
pub(super) const TEST_ANNOTATIONS: &[&str] = &[
    "org.junit.Test",
    "org.junit.jupiter.api.Test",
    "org.testng.annotations.Test",
];

/// Annotations that disable a test.
pub(super) const DISABLE_ANNOTATIONS: &[&str] = &[
    "org.junit.Ignore",
    "org.junit.jupiter.api.Disabled",
    "org.testng.annotations.Ignore",
];

/// The `@Test` annotation of a method, if it is a JUnit/TestNG test;
/// `known` memoizes, per annotation name as written, whether it is one of
/// [`TEST_ANNOTATIONS`] (the answer depends on the name and the file's imports only).
pub(super) fn test_annotation<'t>(
    ctx: &Ctx<'t>,
    method: Node<'t>,
    known: &mut FxHashMap<&'t str, bool>,
) -> Option<Node<'t>> {
    let src = ctx.source();
    syntax::annotations(method).find(|a| {
        syntax::annotation_name(*a, src) == "Test"
            && a.field(Field::Name).is_some_and(|name| {
                *known
                    .entry(ctx.text(name))
                    .or_insert_with(|| annotation_is(ctx, *a, TEST_ANNOTATIONS))
            })
    })
}

/// Whether a method carries `@Disabled` / `@Ignore` (JUnit or TestNG).
pub(super) fn is_disabled_test(ctx: &Ctx<'_>, method: Node<'_>) -> bool {
    let src = ctx.source();
    syntax::annotations(method).any(|a| {
        matches!(syntax::annotation_name(a, src), "Disabled" | "Ignore")
            && annotation_is(ctx, a, DISABLE_ANNOTATIONS)
    })
}

/// The field declaration (and its declarator) named `name` declared directly in the type
/// whose body is `body`.
pub(super) fn find_field<'t>(
    ctx: &Ctx<'t>,
    body: Node<'t>,
    name: &str,
) -> Option<(Node<'t>, Node<'t>)> {
    ctx.members(body)
        .filter(|m| matches!(kind(*m), "field_declaration" | "constant_declaration"))
        .find_map(|m| {
            syntax::declarators(m)
                .find(|d| syntax::name_of(*d, ctx.source()) == name)
                .map(|d| (m, d))
        })
}

/// Whether the method declarations of type body `body` include one named `name`.
pub(super) fn declares_method<'t>(ctx: &Ctx<'t>, body: Node<'t>, name: &str) -> bool {
    ctx.members(body)
        .any(|m| kind(m) == "method_declaration" && syntax::name_of(m, ctx.source()) == name)
}

/// For `x` or `this.x`: the field name `x` and whether it was qualified with `this`.
pub(super) fn simple_or_this_field(expr: Node<'_>) -> Option<(Node<'_>, bool)> {
    let expr = syntax::strip_parens(expr);
    match kind(expr) {
        "identifier" => Some((expr, false)),
        "field_access" => {
            let obj = expr.field(Field::Object)?;
            let field = expr.field(Field::Field)?;
            (kind(obj) == "this" && kind(field) == "identifier").then_some((field, true))
        }
        _ => None,
    }
}
