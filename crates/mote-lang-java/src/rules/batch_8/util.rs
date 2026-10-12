//! Helpers shared by the batch-8 rules: bounded subtree visits, call arguments, test-library
//! recognition and loop-target helpers. Nothing here walks the whole tree.

use crate::prelude::*;

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

/// Whether code inside `n` runs in another function or object (lambda, class body, local or
/// anonymous class): visits of one method body stop there.
pub(super) fn is_nested_scope(n: Node<'_>) -> bool {
    matches!(
        kind(n),
        "lambda_expression"
            | "class_body"
            | "class_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "record_declaration"
            | "annotation_type_declaration"
    )
}

/// Whether any named node of the subtree (the root excluded) satisfies `pred`, not entering
/// lambdas and class bodies.
pub(super) fn any_in_scope<'t>(root: Node<'t>, mut pred: impl FnMut(Node<'t>) -> bool) -> bool {
    visit(root, |n| {
        if is_nested_scope(n) {
            Walk::Skip
        } else if pred(n) {
            Walk::Stop
        } else {
            Walk::Continue
        }
    })
}

/// The argument expressions of a call or object creation (comments skipped).
pub(super) fn args(call: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    call.field(Field::Arguments)
        .into_iter()
        .flat_map(named_children)
}

/// Number of arguments of a call or object creation.
pub(super) fn arg_count(call: Node<'_>) -> usize {
    args(call).count()
}

/// The `i`-th argument of a call or object creation.
pub(super) fn arg(call: Node<'_>, i: usize) -> Option<Node<'_>> {
    args(call).nth(i)
}

/// Whether `expr` names the type `fqn` (the receiver of a static call like `Math.abs`).
pub(super) fn names_type<'t>(ctx: &Ctx<'t>, expr: Node<'t>, fqn: &str) -> bool {
    let simple = fqn.rsplit('.').next().unwrap_or(fqn);
    let text = ctx.text(expr);
    if text != simple && text != fqn {
        return false;
    }
    ctx.type_name_of(expr).as_deref() == Some(fqn)
}

/// Whether the method declares `equals(Object)`-like: named `equals` with one parameter.
pub(super) fn is_equals_decl(ctx: &Ctx<'_>, m: Node<'_>) -> bool {
    kind(m) == "method_declaration"
        && syntax::name_of(m, ctx.source()) == "equals"
        && syntax::parameters(m).count() == 1
}

/// The loop statement a `break` (or `continue`) leaves, following labels; `None` when it
/// targets a `switch` or when the target cannot be found within the function.
pub(super) fn jump_target<'t>(ctx: &Ctx<'t>, jump: Node<'t>) -> Option<Node<'t>> {
    let is_break = kind(jump) == "break_statement";
    let label = named_children(jump)
        .find(|c| kind(*c) == "identifier")
        .map(|l| ctx.text(l));
    for a in ctx.ancestors(jump) {
        match kind(a) {
            "lambda_expression"
            | "method_declaration"
            | "constructor_declaration"
            | "compact_constructor_declaration"
            | "class_body"
            | "static_initializer" => return None,
            "for_statement" | "enhanced_for_statement" | "while_statement" | "do_statement"
                if label.is_none() =>
            {
                return Some(a);
            }
            "switch_block" | "switch_expression" if label.is_none() && is_break => return None,
            "labeled_statement" if label.is_some() => {
                let own = named_children(a)
                    .find(|c| kind(*c) == "identifier")
                    .map(|l| ctx.text(l));
                if own == label {
                    return named_children(a).find(|c| kind(*c) != "identifier");
                }
            }
            _ => {}
        }
    }
    None
}

/// Package prefixes of test assertion libraries.
pub(super) const ASSERTION_PACKAGES: &[&str] = &[
    "org.junit.",
    "junit.framework.",
    "org.assertj.",
    "org.hamcrest.",
    "org.testng.",
    "com.google.common.truth.",
];

/// The public assertion entry points of the test libraries: only calls of these classes are
/// test assertions (a library's own internal helpers named `assert…` are not).
pub(super) const ASSERTION_CLASSES: &[&str] = &[
    "org.junit.Assert",
    "org.junit.jupiter.api.Assertions",
    "junit.framework.Assert",
    "junit.framework.TestCase",
    "org.assertj.core.api.Assertions",
    "org.assertj.core.api.AssertionsForClassTypes",
    "org.assertj.core.api.AssertionsForInterfaceTypes",
    "org.assertj.core.api.BDDAssertions",
    "org.assertj.core.api.Java6Assertions",
    "org.assertj.core.api.Java6BDDAssertions",
    "org.assertj.core.api.Fail",
    "org.hamcrest.MatcherAssert",
    "org.hamcrest.junit.MatcherAssert",
    "org.testng.Assert",
    "org.testng.AssertJUnit",
    "com.google.common.truth.Truth",
    "com.google.common.truth.Truth8",
];

/// Whether an unqualified call `name(..)` is statically imported from a test library's
/// assertion class, or a qualified call is made on one (`Assert.x`, `Assertions.x`).
pub(super) fn is_test_library_call<'t>(ctx: &Ctx<'t>, call: Node<'t>, name: &str) -> bool {
    match call.field(Field::Object) {
        None => {
            if ctx
                .enclosing_class(call)
                .and_then(syntax::type_body)
                .is_some_and(|b| {
                    ctx.members(b).any(|m| {
                        kind(m) == "method_declaration" && syntax::name_of(m, ctx.source()) == name
                    })
                })
            {
                return false;
            }
            ctx.imports().iter().any(|i| {
                if !i.is_static {
                    return false;
                }
                // The class: `a.b.Assert` of `import static a.b.Assert.*` and of
                // `import static a.b.Assert.x`.
                (i.is_wildcard || i.simple_name() == name)
                    && ASSERTION_CLASSES.contains(&i.qualifier())
            })
        }
        Some(obj) => {
            let text = ctx.text(obj);
            if !matches!(
                text,
                "Assert"
                    | "Assertions"
                    | "MatcherAssert"
                    | "TestCase"
                    | "AssertJUnit"
                    | "Truth"
                    | "org.junit.Assert"
                    | "org.junit.jupiter.api.Assertions"
                    | "org.assertj.core.api.Assertions"
            ) {
                return false;
            }
            ctx.type_name_of(obj)
                .is_some_and(|f| ASSERTION_CLASSES.contains(&f.as_str()))
        }
    }
}

/// Whether `ctor` or a type declaration around it carries `@SuppressWarnings("this-escape")`,
/// the name under which javac (`-Xlint:this-escape`) reports a constructor leaking `this`: the
/// author has already looked at the escape and accepted it.
pub(super) fn this_escape_suppressed<'t>(ctx: &Ctx<'t>, ctor: Node<'t>) -> bool {
    let src = ctx.source();
    let suppresses = |decl: Node<'t>| {
        syntax::annotations(decl).any(|a| {
            syntax::annotation_name(a, src) == "SuppressWarnings"
                && ctx.text(a).contains("\"this-escape\"")
        })
    };
    suppresses(ctor)
        || ctx
            .ancestors(ctor)
            .any(|a| syntax::is_type_declaration(a) && suppresses(a))
}
