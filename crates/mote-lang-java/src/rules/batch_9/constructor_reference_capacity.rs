//! `java/constructor-reference-capacity` — `ArrayList::new` (and similar) used as a function of
//! a number, which becomes the initial capacity.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["collections", "lambda"],
    remediation_minutes: 2,
    description: r#"Detects a constructor reference of a collection or string builder with a
capacity constructor — `ArrayList::new`, `HashSet::new`, `HashMap::new`, `StringBuilder::new`,
`ArrayDeque::new`, … — used where it receives a number:

* `map.computeIfAbsent(key, ArrayList::new)` on a map whose keys are `Integer`, `Short`,
  `Byte` or `Character`;
* `intStream.mapToObj(ArrayList::new)`;
* `integerStream.map(HashSet::new)` on a `Stream<Integer>`.

**Why it matters.** The reference resolves to the `int` constructor: the number becomes the
initial capacity, not an element or a key. `computeIfAbsent(userId, ArrayList::new)` allocates
a backing array as large as the id — megabytes for large ids, an
`IllegalArgumentException` for negative ones — and `mapToObj(StringBuilder::new)` yields empty
builders instead of builders holding the number.

**How to fix.** Use a lambda that ignores the argument (`k -> new ArrayList<>()`), or pass the
value explicitly when it was meant as content.

```java
// Noncompliant
byUser.computeIfAbsent(userId, ArrayList::new).add(order);

// Compliant
byUser.computeIfAbsent(userId, k -> new ArrayList<>()).add(order);
```

Only receivers whose key or element type the resolver proves are reported; maps keyed by
strings or collections (where the reference cannot compile or copies a collection) never
are."#,
    ..RuleMeta::new(
        "java/constructor-reference-capacity",
        "java",
        "Constructor reference receives a number as capacity",
    )
};

pub struct ConstructorReferenceCapacity;

/// JDK classes whose constructor taking a single `int` is a capacity or size hint.
const CAPACITY_TYPES: &[&str] = &[
    "java.util.ArrayList",
    "java.util.HashSet",
    "java.util.LinkedHashSet",
    "java.util.HashMap",
    "java.util.LinkedHashMap",
    "java.util.Hashtable",
    "java.util.Vector",
    "java.util.ArrayDeque",
    "java.util.PriorityQueue",
    "java.util.IdentityHashMap",
    "java.util.WeakHashMap",
    "java.util.concurrent.ConcurrentHashMap",
    "java.util.concurrent.ArrayBlockingQueue",
    "java.util.concurrent.LinkedBlockingQueue",
    "java.util.concurrent.LinkedBlockingDeque",
    "java.util.concurrent.PriorityBlockingQueue",
    "java.lang.StringBuilder",
    "java.lang.StringBuffer",
];

const INT_LIKE: &[&str] = &[
    "java.lang.Integer",
    "java.lang.Short",
    "java.lang.Byte",
    "java.lang.Character",
];

/// The simple name of the class of a capacity constructor reference `X::new`.
fn capacity_constructor<'t>(ctx: &Ctx<'t>, mref: Node<'t>) -> Option<&'static str> {
    if kind(mref) != "method_reference" || !ctx.text(mref).ends_with("::new") {
        return None;
    }
    let ty = syntax::first_named_child(mref)?;
    let base = if kind(ty) == "generic_type" {
        syntax::first_named_child(ty)?
    } else {
        ty
    };
    let simple = util::last_segment(ctx.text(base));
    let fqn = *CAPACITY_TYPES
        .iter()
        .find(|f| f.ends_with(&format!(".{simple}")))?;
    let resolved = match kind(base) {
        "identifier" | "field_access" | "scoped_identifier" => ctx.type_name_of(base),
        _ => ctx
            .resolve_type(base)
            .and_then(|t| t.fqn().map(str::to_string)),
    };
    (resolved.as_deref() == Some(fqn)).then(|| util::last_segment(fqn))
}

impl<'t> JavaRule<'t> for ConstructorReferenceCapacity {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["computeIfAbsent", "mapToObj", "map"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) = ctx.call_name(node) else {
            return;
        };
        let (want_args, fn_index) = match name {
            "computeIfAbsent" => (2, 1),
            "mapToObj" | "map" => (1, 0),
            _ => return,
        };
        if util::arg_count(node) != want_args {
            return;
        }
        let Some(f) = util::arg(node, fn_index) else {
            return;
        };
        if kind(f) != "method_reference" || !ctx.text(f).ends_with("::new") {
            return;
        }
        let Some(obj) = node.field(Field::Object) else {
            return;
        };
        let Some(class) = capacity_constructor(ctx, f) else {
            return;
        };
        let Some(t) = ctx.type_of(obj) else {
            return;
        };
        let what = match name {
            "computeIfAbsent" => {
                let Some(map) = ctx.as_supertype(&t, "java.util.Map") else {
                    return;
                };
                if !map
                    .type_arg(0)
                    .is_some_and(|k| INT_LIKE.iter().any(|f| k.is_class(f)))
                {
                    return;
                }
                "the map key"
            }
            "mapToObj" => {
                if !t.is_class("java.util.stream.IntStream") {
                    return;
                }
                "each int"
            }
            _ => {
                let Some(stream) = ctx.as_supertype(&t, "java.util.stream.Stream") else {
                    return;
                };
                if !stream
                    .type_arg(0)
                    .is_some_and(|e| INT_LIKE.iter().any(|f| e.is_class(f)))
                {
                    return;
                }
                "each element"
            }
        };
        ctx.emit(
            f,
            format!("Replace `{}` with a lambda: {what} is passed to the `{class}(int)` constructor as an initial capacity", ctx.text(f)),
        );
    }
}
