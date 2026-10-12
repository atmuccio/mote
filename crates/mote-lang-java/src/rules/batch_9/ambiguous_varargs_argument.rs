//! `java/ambiguous-varargs-argument` — `null` or a primitive array passed alone where a
//! variable-arity parameter is expected.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["varargs", "null"],
    remediation_minutes: 5,
    description: r#"Detects a single argument in a variable-arity (`...`) position that does not
become the one element it looks like:

* a bare `null` passed as the only variable argument of a method declared in the same class
  (`log("x", null)` for `log(String msg, Object... args)`) or of `Arrays.asList`: Java passes
  it as the array itself, so the method receives a `null` array, not an array holding `null`;
* a primitive array (`int[]`, `byte[]`, …) passed alone to `Arrays.asList`, `List.of`,
  `Set.of` or `Stream.of` whose result is then searched or counted (`.contains(..)`,
  `.indexOf(..)`, `.size()`, `.count()`) or looped over with a non-array variable:
  primitives cannot be the elements of a generic list, so the result has exactly one
  element — the array.

**Why it matters.** The callee then throws a `NullPointerException` when it reads the array's
length or loops over it (`Arrays.asList(null)` always does), and a one-element list of an
array makes `contains` always false and `size` always 1.

**How to fix.** Cast `null` to state the intent — `(Object) null` for an array with one
`null`, `(Object[]) null` for no array — or pass an empty array; for primitive arrays use
`Arrays.stream(values).boxed()` or `IntStream.of(values)`.

```java
// Noncompliant
boolean known = Arrays.asList(ids).contains(id);   // ids is an int[]
format("value: %s", null);

// Compliant
boolean known = Arrays.stream(ids).anyMatch(i -> i == id);
format("value: %s", (Object) null);
```

Not reported: a primitive array whose one-element list is stored, returned or passed on
(`List<int[]> rows = List.of(row)` can be intended); methods of other classes (their
overloads are unknown), classes with a superclass or interfaces (an inherited overload or
default method could take the call), overloaded methods, callees that check the array for
`null` themselves, and `Method.invoke(target, null)`, where a `null` array means "no
arguments" by design."#,
    ..RuleMeta::new(
        "java/ambiguous-varargs-argument",
        "java",
        "Ambiguous argument to a varargs parameter",
    )
};

#[derive(Default)]
pub struct AmbiguousVarargsArgument<'t> {
    /// Bare `null` arguments of method calls: the literal, its argument list and the call.
    null_args: Vec<(Node<'t>, Node<'t>, Node<'t>)>,
    /// Names of the methods of this file with a variable-arity parameter.
    varargs_methods: Vec<&'t str>,
}

/// JDK factories with a generic `T...` (or single `E`) parameter.
const GENERIC_FACTORIES: &[(&str, &str)] = &[
    ("java.util.Arrays", "asList"),
    ("java.util.List", "of"),
    ("java.util.Set", "of"),
    ("java.util.stream.Stream", "of"),
];

impl<'t> JavaRule<'t> for AmbiguousVarargsArgument<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation", "null_literal", "spread_parameter"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["asList", "of"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        match kind(node) {
            "null_literal" => {
                self.collect_null(node, ctx);
                return;
            }
            "spread_parameter" => {
                let method = ctx
                    .parent(node)
                    .and_then(|p| ctx.parent(p))
                    .filter(|m| kind(*m) == "method_declaration");
                if let Some(m) = method {
                    self.varargs_methods.push(syntax::name_of(m, ctx.source()));
                }
                return;
            }
            _ => {}
        }
        let Some(name) = ctx.call_name(node) else {
            return;
        };
        if util::arg_count(node) != 1 {
            return;
        }
        let (Some(obj), Some(arg)) = (node.field(Field::Object), util::arg(node, 0)) else {
            return;
        };
        let Some(&(owner, _)) = GENERIC_FACTORIES.iter().find(|(fqn, m)| {
            *m == name && util::last_segment(fqn) == util::last_segment(ctx.text(obj))
        }) else {
            return;
        };
        let is_null = syntax::is_null_literal(arg);
        let wanted = (is_null && name == "asList") || maybe_primitive_array(ctx, arg);
        if !wanted {
            return;
        }
        if !util::names_type(ctx, obj, owner) {
            return;
        }
        let what = util::last_segment(owner);
        if is_null {
            if name == "asList" {
                ctx.emit(
                    arg,
                    "Remove this `null`: `Arrays.asList(null)` passes a null array and always throws NullPointerException; use `Collections.singletonList(null)` for a list holding null",
                );
            }
            return;
        }
        // Only where the elements are clearly what is used: a declared `List<int[]>` or a
        // returned value may well mean the one-array list.
        if !used_elementwise(ctx, node) {
            return;
        }
        let Some(t) = ctx.type_of(arg) else {
            return;
        };
        let Some(elem) = t.element_type() else {
            return;
        };
        if !elem.is_primitive() {
            return;
        }
        ctx.emit(
            arg,
            format!("Box the elements first: `{what}.{name}(..)` given a `{t}` yields one element, the array itself; use `Arrays.stream(..).boxed()`"),
        );
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        if self.varargs_methods.is_empty() {
            return;
        }
        for &(null, list, call) in &self.null_args {
            // The last argument: only `)` (and spaces) follow it in the list.
            let rest = ctx
                .source()
                .get(null.end_byte()..list.end_byte())
                .unwrap_or("");
            if rest.trim_start() != ")" {
                continue;
            }
            let unqualified = match call.field(Field::Object) {
                None => true,
                Some(o) => kind(o) == "this",
            };
            if unqualified
                && ctx
                    .call_name(call)
                    .is_some_and(|n| self.varargs_methods.contains(&n))
            {
                check_null_call(ctx, call);
            }
        }
    }
}

impl<'t> AmbiguousVarargsArgument<'t> {
    /// Records a `null` that is an argument of a method call (filtered in `finish`).
    fn collect_null(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let mut up = ctx.ancestors(node);
        let Some(list) = up.next() else {
            return;
        };
        if kind(list) != "argument_list" {
            return;
        }
        let Some(call) = up.next() else {
            return;
        };
        if kind(call) == "method_invocation" {
            self.null_args.push((node, list, call));
        }
    }
}

const PRIMITIVE_ARRAYS: &[&str] = &[
    "int[]",
    "long[]",
    "short[]",
    "byte[]",
    "char[]",
    "float[]",
    "double[]",
    "boolean[]",
];

/// Cheap syntactic test before asking the resolver: a variable or field declared (somewhere
/// in the file) with a primitive (array) type, or a primitive array creation.
fn maybe_primitive_array<'t>(ctx: &Ctx<'t>, arg: Node<'t>) -> bool {
    let a = syntax::strip_parens(arg);
    let declared_primitive = |name: Node<'t>| {
        ctx.declared_types().types_of(ctx.text(name)).any(|t| {
            let text = ctx.text(t);
            PRIMITIVE_ARRAYS.contains(&text)
                || matches!(
                    kind(t),
                    "integral_type" | "floating_point_type" | "boolean_type"
                )
        })
    };
    match kind(a) {
        "identifier" => declared_primitive(a),
        "field_access" => a.field(Field::Field).is_some_and(declared_primitive),
        "array_creation_expression" => a.field(Field::Type).is_some_and(|t| {
            matches!(
                kind(t),
                "integral_type" | "floating_point_type" | "boolean_type"
            )
        }),
        _ => false,
    }
}

fn check_null_call<'t>(ctx: &Ctx<'t>, call: Node<'t>) {
    let src = ctx.source();
    let Some(name) = ctx.call_name(call) else {
        return;
    };
    let n = util::arg_count(call);
    // The method must be declared in the innermost class around the call, which has no
    // superclass, and must be its only method of that name.
    let Some(class) = ctx.enclosing_class(call) else {
        return;
    };
    if kind(class) != "class_declaration"
        || class.field(Field::Superclass).is_some()
        || class.field(Field::Interfaces).is_some()
    {
        return;
    }
    let Some(body) = class.field(Field::Body) else {
        return;
    };
    let mut found = None;
    for m in ctx.members(body) {
        if kind(m) == "method_declaration" && syntax::name_of(m, src) == name {
            if found.is_some() {
                return;
            }
            found = Some(m);
        }
    }
    let Some(method) = found else {
        return;
    };
    let params: Vec<Node<'t>> = syntax::parameters(method).collect();
    let Some(last) = params.last() else {
        return;
    };
    if kind(*last) != "spread_parameter" || params.len() != n {
        return;
    }
    let Some(pname) = syntax::child_of_kind(*last, "variable_declarator")
        .and_then(|d| d.field(Field::Name))
        .map(|id| ctx.text(id))
    else {
        return;
    };
    let elem = syntax::first_named_child(*last)
        .map(|t| ctx.text(t))
        .unwrap_or("Object");
    // A callee checking its array for null expects this call.
    let Some(mbody) = method.field(Field::Body) else {
        return;
    };
    let text = ctx.text(mbody);
    let checks_null = [
        format!("{pname} == null"),
        format!("{pname} != null"),
        format!("null == {pname}"),
        format!("null != {pname}"),
        format!("{pname}==null"),
        format!("{pname}!=null"),
        format!("isNull({pname})"),
        format!("nonNull({pname})"),
        format!("requireNonNull({pname}"),
        format!("isEmpty({pname})"),
        format!("isNotEmpty({pname})"),
    ]
    .iter()
    .any(|p| text.contains(p.as_str()));
    if checks_null {
        return;
    }
    let Some(arg) = util::arg(call, n.saturating_sub(1)) else {
        return;
    };
    ctx.emit(
        arg,
        format!("Cast this `null`: `{name}` receives it as a null `{pname}` array, not as one null element; write `({elem}) null` or pass an empty array"),
    );
}

/// Methods whose use on the result shows the elements were meant to be the array's values.
const ELEMENT_USES: &[&str] = &[
    "contains",
    "containsAll",
    "indexOf",
    "lastIndexOf",
    "size",
    "count",
];

/// Whether the result of `call` is used element by element: a call like `.contains(..)` or
/// `.size()` chained directly on it, or it is the iterable of an enhanced `for`.
fn used_elementwise<'t>(ctx: &Ctx<'t>, call: Node<'t>) -> bool {
    let Some(parent) = ctx.parent(call) else {
        return false;
    };
    match kind(parent) {
        "method_invocation" => {
            parent.field(Field::Object).map(|o| o.id()) == Some(call.id())
                && ctx
                    .call_name(parent)
                    .is_some_and(|n| ELEMENT_USES.contains(&n))
        }
        "enhanced_for_statement" => {
            // `for (Object o : Arrays.asList(ints))`: a loop over values, run once on the array.
            parent.field(Field::Value).map(|v| v.id()) == Some(call.id())
                && parent
                    .field(Field::Type)
                    .is_some_and(|t| kind(t) != "array_type" && ctx.text(t) != "var")
        }
        _ => false,
    }
}
