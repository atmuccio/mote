//! `java/explicit-varargs-array` — a new array built only to be passed to a varargs parameter.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["varargs", "clutter"],
    remediation_minutes: 2,
    description: r#"Detects an array created in the argument list (`new Object[] { a, b }`,
`new Class[0]`) and passed as the variable-arity (`...`) argument of a JDK method —
`String.format`, `printf`, `MessageFormat.format`, `Arrays.asList`, `List.of`, `Stream.of`,
`Paths.get`, `Method.invoke`, `Class.getMethod`, … — or of a method declared in the same class.

**Why it matters.** A varargs parameter builds that array itself: writing it out adds noise
(`new Object[] { … }` around the real arguments) and makes a call look like it needs an array
when it does not. It is usually a leftover of code written before Java 5 or of a library
version whose method took an array.

**How to fix.** Pass the elements directly, or nothing for an empty array.

```java
// Noncompliant
String msg = String.format("%s: %d", new Object[] { name, count });
Method m = type.getMethod("run", new Class[0]);

// Compliant
String msg = String.format("%s: %d", name, count);
Method m = type.getMethod("run");
```

Not reported: an array with a single element that could itself be an array or `null`
(unwrapping `invoke(target, new Object[] { args })` would spread `args` instead of passing it
as one argument), primitive arrays given to `Object...` or generic parameters (they are one
element there, see `java/ambiguous-varargs-argument`), methods of libraries (their parameter
may be a plain array in the version used), and same-class methods that are overloaded or
declared in a class that extends or implements anything (an inherited overload could take the
call)."#,
    ..RuleMeta::new(
        "java/explicit-varargs-array",
        "java",
        "Array created for a varargs parameter",
    )
};

/// What the variable-arity parameter holds.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Elem {
    /// `Object...`: one element that is an array or `null` would be spread.
    Object,
    /// `T...`
    Generic,
    /// A specific reference type (`String...`, `Class<?>...`).
    Ref,
    /// A primitive type (`int...`).
    Prim,
}

struct Api {
    owner: &'static str,
    name: &'static str,
    /// Allowed numbers of arguments before the varargs one.
    fixed: &'static [usize],
    elem: Elem,
    /// Only a call through the class name is the varargs method (`MessageFormat.format`).
    static_only: bool,
}

const fn api(owner: &'static str, name: &'static str, fixed: &'static [usize], elem: Elem) -> Api {
    Api {
        owner,
        name,
        fixed,
        elem,
        static_only: false,
    }
}

const APIS: &[Api] = &[
    api("java.lang.String", "format", &[1, 2], Elem::Object),
    api("java.lang.String", "formatted", &[0], Elem::Object),
    api("java.lang.String", "join", &[1], Elem::Ref),
    Api {
        owner: "java.text.MessageFormat",
        name: "format",
        fixed: &[1],
        elem: Elem::Object,
        static_only: true,
    },
    api("java.io.PrintStream", "printf", &[1, 2], Elem::Object),
    api("java.io.PrintStream", "format", &[1, 2], Elem::Object),
    api("java.io.PrintWriter", "printf", &[1, 2], Elem::Object),
    api("java.io.PrintWriter", "format", &[1, 2], Elem::Object),
    api("java.util.Arrays", "asList", &[0], Elem::Generic),
    api("java.util.List", "of", &[0], Elem::Generic),
    api("java.util.Set", "of", &[0], Elem::Generic),
    api("java.util.stream.Stream", "of", &[0], Elem::Generic),
    api("java.util.stream.IntStream", "of", &[0], Elem::Prim),
    api("java.util.stream.LongStream", "of", &[0], Elem::Prim),
    api("java.util.stream.DoubleStream", "of", &[0], Elem::Prim),
    api("java.util.Collections", "addAll", &[1], Elem::Generic),
    api("java.util.Objects", "hash", &[0], Elem::Object),
    api("java.nio.file.Paths", "get", &[1], Elem::Ref),
    api("java.nio.file.Path", "of", &[1], Elem::Ref),
    api("java.lang.Class", "getMethod", &[1], Elem::Ref),
    api("java.lang.Class", "getDeclaredMethod", &[1], Elem::Ref),
    api("java.lang.Class", "getConstructor", &[0], Elem::Ref),
    api("java.lang.Class", "getDeclaredConstructor", &[0], Elem::Ref),
    api("java.lang.reflect.Method", "invoke", &[1], Elem::Object),
    api(
        "java.lang.reflect.Constructor",
        "newInstance",
        &[0],
        Elem::Object,
    ),
    api(
        "java.util.concurrent.CompletableFuture",
        "allOf",
        &[0],
        Elem::Ref,
    ),
    api(
        "java.util.concurrent.CompletableFuture",
        "anyOf",
        &[0],
        Elem::Ref,
    ),
];

/// A same-file method with a variable-arity parameter.
struct VarargsMethod<'t> {
    name: &'t str,
    /// Id of the declaring class node.
    class: usize,
    params: usize,
    elem: Elem,
}

#[derive(Default)]
pub struct ExplicitVarargsArray<'t> {
    /// Every method of this file (examined in `finish` only when a candidate call exists).
    methods: Vec<Node<'t>>,
    /// Array creations passed last to an unqualified call in a class that inherits nothing.
    pending: Vec<Pending<'t>>,
}

impl<'t> JavaRule<'t> for ExplicitVarargsArray<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["array_creation_expression", "method_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if kind(node) == "method_declaration" {
            self.methods.push(node);
            return;
        }
        let Some(args) = ctx.parent(node) else {
            return;
        };
        if kind(args) != "argument_list" {
            return;
        }
        let Some(elem_count) = element_count(ctx, node) else {
            return;
        };
        // The array must be the last argument.
        if syntax::named_children(args)
            .filter(|n| !syntax::is_comment(*n))
            .last()
            .map(|n| n.id())
            != Some(node.id())
        {
            return;
        }
        let Some(call) = ctx.parent(args) else {
            return;
        };
        if kind(call) != "method_invocation" {
            return;
        }
        let Some(name) = ctx.call_name(call) else {
            return;
        };
        let fixed = util::arg_count(call) - 1;
        let recv = call.field(Field::Object);
        if recv.is_none_or(|r| kind(r) == "this") {
            if let Some(class) = ctx.enclosing_class(call).filter(|c| standalone(*c)) {
                self.pending.push(Pending {
                    array: node,
                    call,
                    class: class.id(),
                    ref_ok: elements_ok(ctx, node, Elem::Object),
                    prim_ok: elements_ok(ctx, node, Elem::Prim),
                });
            }
        }
        if !APIS
            .iter()
            .any(|a| a.name == name && a.fixed.contains(&fixed))
        {
            return;
        }
        let api = APIS.iter().find(|a| {
            a.name == name
                && a.fixed.contains(&fixed)
                && if a.static_only {
                    recv.is_some_and(|r| util::names_type(ctx, r, a.owner))
                } else {
                    ctx.is_call_to(call, a.owner, name)
                }
        });
        if api.is_some_and(|a| elements_ok(ctx, node, a.elem)) {
            report(ctx, node, name, elem_count);
        }
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        if self.pending.is_empty() {
            return;
        }
        // Number of methods per (class, name) and the varargs methods.
        let mut method_names: FxHashMap<(usize, &'t str), u32> = FxHashMap::default();
        let mut varargs: Vec<VarargsMethod<'t>> = Vec::new();
        for &m in &self.methods {
            let Some(class) = ctx.enclosing_class(m).map(|c| c.id()) else {
                continue;
            };
            let name = syntax::name_of(m, ctx.source());
            *method_names.entry((class, name)).or_default() += 1;
            if let Some(v) = varargs_method(ctx, m, name, class) {
                varargs.push(v);
            }
        }
        for p in &self.pending {
            let Some(name) = ctx.call_name(p.call) else {
                continue;
            };
            // The call is made in the declaring class (which inherits nothing that could be an
            // overload), and that class declares no other method of this name.
            if method_names.get(&(p.class, name)).copied() != Some(1) {
                continue;
            }
            let Some(m) = varargs
                .iter()
                .find(|m| m.class == p.class && m.name == name)
            else {
                continue;
            };
            if util::arg_count(p.call) != m.params {
                continue;
            }
            let (array, ref_ok, prim_ok) = (p.array, p.ref_ok, p.prim_ok);
            if !(if m.elem == Elem::Prim {
                prim_ok
            } else {
                ref_ok
            }) {
                continue;
            }
            if let Some(n) = element_count(ctx, array) {
                report(ctx, array, name, n);
            }
        }
    }
}

/// The varargs description of a method whose last parameter is `T... name`.
fn varargs_method<'t>(
    ctx: &Ctx<'t>,
    m: Node<'t>,
    name: &'t str,
    class: usize,
) -> Option<VarargsMethod<'t>> {
    let params: Vec<Node<'t>> = syntax::parameters(m).collect();
    let &last = params.last()?;
    if kind(last) != "spread_parameter" {
        return None;
    }
    let ty = named_children(last).find(|c| kind(*c) != "modifiers")?;
    let tparams: Vec<&str> = m
        .field(Field::TypeParameters)
        .into_iter()
        .flat_map(named_children)
        .filter_map(|tp| named_children(tp).find(|c| kind(*c) == "type_identifier"))
        .map(|id| ctx.text(id))
        .collect();
    let text = ctx.text(ty);
    let elem = match kind(ty) {
        "integral_type" | "floating_point_type" | "boolean_type" => Elem::Prim,
        _ if text == "Object" || text == "java.lang.Object" => Elem::Object,
        _ if tparams.contains(&text) => Elem::Generic,
        // A class type parameter or anything unclear: treat like a generic.
        "type_identifier" if text.len() == 1 => Elem::Generic,
        _ => Elem::Ref,
    };
    Some(VarargsMethod {
        name,
        class,
        params: params.len(),
        elem,
    })
}

/// A class, enum or record that neither extends nor implements anything.
fn standalone(class: Node<'_>) -> bool {
    match kind(class) {
        "class_declaration" => {
            class.field(Field::Superclass).is_none() && class.field(Field::Interfaces).is_none()
        }
        "enum_declaration" | "record_declaration" => class.field(Field::Interfaces).is_none(),
        _ => false,
    }
}

/// The number of elements of a one-dimensional array creation `new T[] { … }` or `new T[0]`.
fn element_count<'t>(ctx: &Ctx<'t>, array: Node<'t>) -> Option<usize> {
    let mut dims =
        named_children(array).filter(|c| matches!(kind(*c), "dimensions" | "dimensions_expr"));
    let (Some(d), None) = (dims.next(), dims.next()) else {
        return None;
    };
    match kind(d) {
        "dimensions" => {
            let compact: String = ctx.text(d).chars().filter(|c| !c.is_whitespace()).collect();
            if compact != "[]" {
                return None;
            }
            let init = array.field(Field::Value)?;
            if kind(init) != "array_initializer" {
                return None;
            }
            Some(
                named_children(init)
                    .filter(|n| !syntax::is_comment(*n))
                    .count(),
            )
        }
        _ => {
            let size = syntax::first_named_child(d)?;
            (syntax::int_literal_value(size, ctx.source()) == Some(0)).then_some(0)
        }
    }
}

/// Whether unwrapping the array keeps the meaning of the call for this kind of parameter.
fn elements_ok<'t>(ctx: &Ctx<'t>, array: Node<'t>, elem: Elem) -> bool {
    let Some(ty) = array.field(Field::Type) else {
        return false;
    };
    let primitive = matches!(
        kind(ty),
        "integral_type" | "floating_point_type" | "boolean_type"
    );
    match elem {
        Elem::Prim => {
            if !primitive {
                return false;
            }
        }
        _ => {
            if primitive {
                return false;
            }
        }
    }
    let Some(init) = array.field(Field::Value) else {
        return true; // `new T[0]`
    };
    let mut elems = named_children(init).filter(|n| !syntax::is_comment(*n));
    let (Some(only), None) = (elems.next(), elems.next()) else {
        return true; // zero or several elements
    };
    if elem == Elem::Prim {
        return true;
    }
    if syntax::is_null_literal(syntax::strip_parens(only)) {
        return false;
    }
    ctx.type_of(only)
        .is_some_and(|t| !t.is_array() && !t.is_null() && t.is_fully_known())
}

fn report<'t>(ctx: &Ctx<'t>, array: Node<'t>, name: &str, elems: usize) {
    let end = named_children(array)
        .find(|c| matches!(kind(*c), "dimensions" | "dimensions_expr"))
        .map_or(array.end_byte(), |d| d.end_byte());
    let msg = if elems == 0 {
        format!("Remove this empty array: `{name}` takes a variable number of arguments")
    } else {
        format!("Pass the elements directly: `{name}` takes a variable number of arguments")
    };
    ctx.emit_range(Span::new(array.start_byte(), end), msg);
}

/// An array creation passed last to an unqualified call made in a class that inherits
/// nothing, with whether unwrapping it is safe for a reference and for a primitive parameter
/// (computed during the walk).
struct Pending<'t> {
    array: Node<'t>,
    call: Node<'t>,
    class: usize,
    ref_ok: bool,
    prim_ok: bool,
}
