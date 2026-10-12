//! `java/anonymous-class-could-be-lambda` — an anonymous class implementing a functional
//! interface with a single method that a lambda expresses.

use super::util::{self, Walk};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["idiom", "readability", "java8"],
    remediation_minutes: 5,
    description: r#"Detects an anonymous class that implements a functional interface — a JDK one
such as `Runnable`, `Callable`, `Comparator`, the `java.util.function` types, `ThreadFactory`
or `ActionListener`, or an interface of the same file with a single abstract method — by
defining just that method, with no fields, no other members and no use of `this`.

**Why it matters.** Since Java 8 a lambda states the same behavior without the class
declaration, the repeated method signature and the extra indentation: the reader sees the
code that runs instead of the boilerplate around it. An anonymous class also always creates a
new object and, in an instance context, keeps a reference to the enclosing object even when
it doesn't use it; a lambda that captures nothing is a shared constant and one that doesn't
use `this` holds no reference to the enclosing object.

**How to fix.** Replace the anonymous class with a lambda (or a method reference).

```java
// Noncompliant
executor.execute(new Runnable() {
    @Override
    public void run() {
        refresh(cache);
    }
});
names.sort(new Comparator<String>() {
    @Override
    public int compare(String a, String b) {
        return a.length() - b.length();
    }
});

// Compliant
executor.execute(() -> refresh(cache));
names.sort((a, b) -> a.length() - b.length());
```

Only reported for files compiled for Java 8 or later. Not reported: anonymous classes with
fields, initializers or several methods; bodies that use `this` or `super`, call the method
itself recursively, or call methods the object inherits (`toString()`, `hashCode()`, the
interface's default methods), which a lambda would resolve differently; generic methods;
parameters or local variables named like variables of the enclosing method (a lambda cannot
redeclare them); objects whose target type a lambda would not get (a `var` or `Object`
variable, a receiver of a call, an argument of generic methods such as `List.of`,
`Arrays.asList` or collection `add` / `put`); and `PrivilegedAction`, whose lambdas make
`doPrivileged` calls ambiguous."#,
    ..RuleMeta::new(
        "java/anonymous-class-could-be-lambda",
        "java",
        "Anonymous class could be a lambda",
    )
};

pub struct AnonymousClassCouldBeLambda;

/// JDK functional interfaces: FQN, the abstract method, and the other instance methods
/// (default methods) an unqualified call in the body could refer to.
const JDK_FUNCTIONAL: &[(&str, &str, &[&str])] = &[
    ("java.lang.Runnable", "run", &[]),
    ("java.util.concurrent.Callable", "call", &[]),
    (
        "java.util.Comparator",
        "compare",
        &[
            "reversed",
            "thenComparing",
            "thenComparingInt",
            "thenComparingLong",
            "thenComparingDouble",
        ],
    ),
    ("java.util.concurrent.ThreadFactory", "newThread", &[]),
    (
        "java.util.function.Function",
        "apply",
        &["andThen", "compose"],
    ),
    ("java.util.function.BiFunction", "apply", &["andThen"]),
    (
        "java.util.function.UnaryOperator",
        "apply",
        &["andThen", "compose"],
    ),
    ("java.util.function.BinaryOperator", "apply", &["andThen"]),
    ("java.util.function.Supplier", "get", &[]),
    ("java.util.function.Consumer", "accept", &["andThen"]),
    ("java.util.function.BiConsumer", "accept", &["andThen"]),
    (
        "java.util.function.Predicate",
        "test",
        &["and", "or", "negate"],
    ),
    (
        "java.util.function.BiPredicate",
        "test",
        &["and", "or", "negate"],
    ),
    ("java.util.function.IntFunction", "apply", &[]),
    (
        "java.util.function.IntPredicate",
        "test",
        &["and", "or", "negate"],
    ),
    (
        "java.util.function.IntUnaryOperator",
        "applyAsInt",
        &["andThen", "compose"],
    ),
    ("java.util.function.IntBinaryOperator", "applyAsInt", &[]),
    (
        "java.util.function.LongUnaryOperator",
        "applyAsLong",
        &["andThen", "compose"],
    ),
    ("java.util.function.IntConsumer", "accept", &["andThen"]),
    ("java.util.function.ToIntFunction", "applyAsInt", &[]),
    ("java.util.function.ToLongFunction", "applyAsLong", &[]),
    ("java.util.function.ToDoubleFunction", "applyAsDouble", &[]),
    ("java.util.function.BooleanSupplier", "getAsBoolean", &[]),
    ("java.util.function.IntSupplier", "getAsInt", &[]),
    ("java.util.function.LongSupplier", "getAsLong", &[]),
    ("java.io.FileFilter", "accept", &[]),
    ("java.io.FilenameFilter", "accept", &[]),
    (
        "java.lang.Thread.UncaughtExceptionHandler",
        "uncaughtException",
        &[],
    ),
    ("java.awt.event.ActionListener", "actionPerformed", &[]),
    ("java.beans.PropertyChangeListener", "propertyChange", &[]),
    ("javax.swing.event.ChangeListener", "stateChanged", &[]),
    (
        "javax.swing.event.ListSelectionListener",
        "valueChanged",
        &[],
    ),
];

/// Methods every object has: an unqualified call to one of them means the anonymous object.
const OBJECT_METHODS: &[&str] = &[
    "toString",
    "hashCode",
    "equals",
    "getClass",
    "notify",
    "notifyAll",
    "wait",
    "clone",
    "finalize",
];

/// Calls whose parameter is a type variable or `Object`: a lambda argument has no target type.
const GENERIC_SINKS: &[&str] = &[
    "of",
    "ofNullable",
    "asList",
    "singletonList",
    "singleton",
    "singletonMap",
    "requireNonNull",
    "requireNonNullElse",
    "add",
    "addAll",
    "addElement",
    "put",
    "putIfAbsent",
    "offer",
    "offerFirst",
    "offerLast",
    "push",
    "set",
    "addFirst",
    "addLast",
    "valueOf",
    "print",
    "println",
    "format",
    "equals",
    "assertEquals",
    "assertSame",
    "assertNotNull",
];

/// The functional interface an anonymous class implements: its abstract method and the other
/// method names of the interface.
struct Target<'t> {
    method: &'t str,
    others: Vec<&'t str>,
}

/// The JDK functional interface named by `fqn`, or a same-file interface with one abstract
/// method.
fn target<'t>(ctx: &Ctx<'t>, ty: Node<'t>) -> Option<Target<'t>> {
    let fqn = util::type_fqn(ctx, ty)?;
    if let Some(&(_, method, others)) = JDK_FUNCTIONAL.iter().find(|f| f.0 == fqn) {
        return Some(Target {
            method,
            others: others.to_vec(),
        });
    }
    // An interface declared in this file, without supertypes, with one abstract method.
    let simple = util::last_segment(&fqn);
    let decl = find_interface(ctx, ctx.root(), simple, &fqn, 0)?;
    if syntax::child_of_kind(decl, "extends_interfaces").is_some() {
        return None;
    }
    let body = decl.field(Field::Body)?;
    let mut abstract_method = None;
    let mut others = Vec::new();
    for m in named_children(body) {
        if kind(m) != "method_declaration" {
            continue;
        }
        let name = syntax::name_of(m, ctx.source());
        let is_abstract = m.field(Field::Body).is_none()
            && !syntax::has_modifier(m, "static")
            && !syntax::has_modifier(m, "default")
            && !syntax::has_modifier(m, "private");
        if is_abstract {
            if abstract_method.is_some() || m.field(Field::TypeParameters).is_some() {
                return None;
            }
            abstract_method = Some(name);
        } else {
            others.push(name);
        }
    }
    Some(Target {
        method: abstract_method?,
        others,
    })
}

/// The interface declaration named `simple` whose FQN is `fqn`, among the type declarations of
/// `scope` (nested ones included, bounded depth).
fn find_interface<'t>(
    ctx: &Ctx<'t>,
    scope: Node<'t>,
    simple: &str,
    fqn: &str,
    depth: u32,
) -> Option<Node<'t>> {
    if depth > 4 {
        return None;
    }
    let members: Box<dyn Iterator<Item = Node<'t>>> = if kind(scope) == "program" {
        Box::new(named_children(scope))
    } else {
        Box::new(
            syntax::type_body(scope)
                .into_iter()
                .flat_map(|b| ctx.members(b)),
        )
    };
    for m in members {
        if !syntax::is_type_declaration(m) {
            continue;
        }
        if kind(m) == "interface_declaration"
            && syntax::name_of(m, ctx.source()) == simple
            && ctx.class_fqn(m).as_deref() == Some(fqn)
        {
            return Some(m);
        }
        if let Some(found) = find_interface(ctx, m, simple, fqn, depth + 1) {
            return Some(found);
        }
    }
    None
}

/// Whether the object creation is in a position where a lambda gets the interface as its
/// target type.
fn has_target_type<'t>(ctx: &Ctx<'t>, node: Node<'t>) -> bool {
    let Some(parent) = ctx.parent(node) else {
        return false;
    };
    match kind(parent) {
        "argument_list" => {
            let Some(call) = ctx.parent(parent) else {
                return false;
            };
            match kind(call) {
                "method_invocation" => ctx
                    .call_name(call)
                    .is_some_and(|n| !GENERIC_SINKS.contains(&n)),
                "object_creation_expression" | "explicit_constructor_invocation" => true,
                _ => false,
            }
        }
        "variable_declarator" => {
            let Some(decl) = ctx.parent(parent) else {
                return false;
            };
            decl.field(Field::Type).is_some_and(|t| {
                let text = ctx.text(t);
                text != "var" && text != "Object" && text != "java.lang.Object"
            })
        }
        "return_statement" => ctx
            .enclosing_function(node)
            .is_some_and(|f| kind(f) == "method_declaration"),
        _ => false,
    }
}

/// Names declared in the enclosing functions of `node` (parameters and locals, up to the
/// enclosing class), which the lambda's parameters and locals must not redeclare.
fn enclosing_names<'t>(ctx: &Ctx<'t>, node: Node<'t>) -> Vec<&'t str> {
    let mut names = Vec::new();
    for a in ctx.ancestors(node) {
        match kind(a) {
            "class_body" | "interface_body" | "enum_body" | "record_declaration" => break,
            "method_declaration"
            | "constructor_declaration"
            | "compact_constructor_declaration"
            | "lambda_expression"
            | "static_initializer" => collect_declared(ctx, a, &mut names),
            "block" if ctx.parent(a).is_some_and(|p| kind(p) == "class_body") => {
                collect_declared(ctx, a, &mut names)
            }
            _ => {}
        }
    }
    names
}

/// Declared variable names in `root` (not entering class bodies).
fn collect_declared<'t>(ctx: &Ctx<'t>, root: Node<'t>, out: &mut Vec<&'t str>) {
    util::visit(root, |n| {
        match kind(n) {
            "class_body" => return Walk::Skip,
            "variable_declarator"
            | "formal_parameter"
            | "catch_formal_parameter"
            | "enhanced_for_statement"
            | "resource"
            | "spread_parameter" => {
                if let Some(name) = n.field(Field::Name) {
                    out.push(ctx.text(name));
                }
            }
            "inferred_parameters" => {
                for p in named_children(n) {
                    if kind(p) == "identifier" {
                        out.push(ctx.text(p));
                    }
                }
            }
            "lambda_expression" => {
                if let Some(p) = n.field(Field::Parameters) {
                    if kind(p) == "identifier" {
                        out.push(ctx.text(p));
                    }
                }
            }
            "instanceof_expression" => {
                if let Some(name) = n.field(Field::Name) {
                    out.push(ctx.text(name));
                }
            }
            _ => {}
        }
        Walk::Continue
    });
}

impl<'t> JavaRule<'t> for AnonymousClassCouldBeLambda {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["object_creation_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(body) = syntax::child_of_kind(node, "class_body") else {
            return;
        };
        // Exactly one member: a method.
        let mut members = named_children(body);
        let (Some(method), None) = (members.next(), members.next()) else {
            return;
        };
        if kind(method) != "method_declaration"
            || method.field(Field::TypeParameters).is_some()
            || syntax::argument_count(node) != 0
        {
            return;
        }
        if ctx.java_release().is_none_or(|r| r < 8) {
            return;
        }
        let src = ctx.source();
        // Annotations other than `@Override` have no place on a lambda.
        if syntax::annotations(method).any(|a| syntax::annotation_name(a, src) != "Override") {
            return;
        }
        let Some(ty) = node.field(Field::Type) else {
            return;
        };
        let Some(method_body) = method.field(Field::Body) else {
            return;
        };
        let method_name = syntax::name_of(method, src);
        if !has_target_type(ctx, node) {
            return;
        }
        let Some(target) = target(ctx, ty) else {
            return;
        };
        if target.method != method_name {
            return;
        }
        // The body must not refer to the anonymous object.
        let mut declared: Vec<&'t str> = Vec::new();
        for p in syntax::parameters(method) {
            if let Some(n) = p.field(Field::Name) {
                declared.push(ctx.text(n));
            } else {
                return;
            }
        }
        let self_reference = util::visit(method_body, |n| match kind(n) {
            "class_body" => Walk::Skip,
            "this" | "super" => {
                // `Outer.this` is fine in a lambda; a bare `this` / `super` is not.
                Walk::Stop
            }
            "field_access" => {
                let qualified_this = n.field(Field::Field).is_some_and(|f| kind(f) == "this")
                    && n.field(Field::Object).is_some_and(|o| kind(o) != "this");
                if qualified_this {
                    Walk::Skip
                } else {
                    Walk::Continue
                }
            }
            "method_invocation" => {
                if n.field(Field::Object).is_none() {
                    let name = ctx.call_name(n).unwrap_or("");
                    if name.is_empty()
                        || name == method_name
                        || OBJECT_METHODS.contains(&name)
                        || target.others.contains(&name)
                    {
                        return Walk::Stop;
                    }
                }
                Walk::Continue
            }
            _ => Walk::Continue,
        });
        if self_reference {
            return;
        }
        collect_declared(ctx, method_body, &mut declared);
        if !declared.is_empty() {
            let outer = enclosing_names(ctx, node);
            if declared.iter().any(|d| outer.contains(d)) {
                return;
            }
        }
        let type_name = ctx.text(util::type_base(ty));
        ctx.emit_range(
            Span::cover(node, ty),
            format!("Replace this anonymous `{type_name}` with a lambda"),
        );
    }
}
