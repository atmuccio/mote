//! `java/raw-type` — a generic JDK type used without type arguments.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["generics", "type-safety"],
    remediation_minutes: 5,
    description: r#"Detects generic JDK types used without type arguments — `List items`,
`new HashMap()`, `Map<String, List> index`, `Class type` — in the types of fields, local
variables, parameters, method results and `new` expressions, and the untyped constants
`Collections.EMPTY_LIST`, `EMPTY_SET` and `EMPTY_MAP`.

**Why it matters.** A raw type switches off the compiler's checks for everything that goes
through it: any object can be added to a raw `List`, and what comes out has to be cast back,
so a wrong element type is only discovered at run time as a `ClassCastException`, far from
the code that inserted it. Raw types exist only for compatibility with code written before
Java 5.

**How to fix.** Give the type arguments (`List<Order>`, `new HashMap<>()`, `Class<?>` when
any class will do), or use `Collections.emptyList()`, `emptySet()`, `emptyMap()`.

```java
// Noncompliant
List names = new ArrayList();
Map<String, List> byCity = new HashMap<>();
return Collections.EMPTY_LIST;

// Compliant
List<String> names = new ArrayList<>();
Map<String, List<Customer>> byCity = new HashMap<>();
return Collections.emptyList();
```

Off by default: legacy code bases use raw types in large numbers. Not reported: `instanceof`,
class literals and casts (where a raw type is the norm or the point), array types and array
creation (`new List[n]` cannot be parameterized), a raw type argument of a declaration that
passes the matching class literal (`ArgumentCaptor<List> c = ArgumentCaptor.forClass(List.class)`
cannot be written otherwise), supertypes in `extends` / `implements`, parameters and results of methods marked
`@Override` (their signature is dictated by the overridden method), declarations under
`@SuppressWarnings("rawtypes")`, and types other than the JDK's."#,
    ..RuleMeta::new("java/raw-type", "java", "Raw use of a generic type")
};

pub struct RawType;

/// Generic JDK types by simple name (FQN checked through the resolver).
const GENERIC_TYPES: &[&str] = &[
    "java.util.Collection",
    "java.util.List",
    "java.util.ArrayList",
    "java.util.LinkedList",
    "java.util.Vector",
    "java.util.Stack",
    "java.util.Set",
    "java.util.HashSet",
    "java.util.LinkedHashSet",
    "java.util.TreeSet",
    "java.util.SortedSet",
    "java.util.NavigableSet",
    "java.util.EnumSet",
    "java.util.Map",
    "java.util.HashMap",
    "java.util.LinkedHashMap",
    "java.util.TreeMap",
    "java.util.SortedMap",
    "java.util.NavigableMap",
    "java.util.Hashtable",
    "java.util.WeakHashMap",
    "java.util.IdentityHashMap",
    "java.util.EnumMap",
    "java.util.Map.Entry",
    "java.util.Iterator",
    "java.util.ListIterator",
    "java.util.Enumeration",
    "java.util.Comparator",
    "java.util.Queue",
    "java.util.Deque",
    "java.util.ArrayDeque",
    "java.util.PriorityQueue",
    "java.util.Optional",
    "java.util.Dictionary",
    "java.lang.Class",
    "java.lang.Comparable",
    "java.lang.Iterable",
    "java.lang.ThreadLocal",
    "java.lang.InheritableThreadLocal",
    "java.lang.ref.WeakReference",
    "java.lang.ref.SoftReference",
    "java.lang.ref.Reference",
    "java.lang.reflect.Constructor",
    "java.util.concurrent.Future",
    "java.util.concurrent.Callable",
    "java.util.concurrent.ConcurrentMap",
    "java.util.concurrent.ConcurrentHashMap",
    "java.util.concurrent.CopyOnWriteArrayList",
    "java.util.concurrent.CopyOnWriteArraySet",
    "java.util.concurrent.BlockingQueue",
    "java.util.concurrent.LinkedBlockingQueue",
    "java.util.concurrent.ArrayBlockingQueue",
    "java.util.concurrent.ConcurrentLinkedQueue",
    "java.util.concurrent.CompletableFuture",
    "java.util.concurrent.FutureTask",
    "java.util.concurrent.atomic.AtomicReference",
    "java.util.function.Function",
    "java.util.function.BiFunction",
    "java.util.function.Supplier",
    "java.util.function.Consumer",
    "java.util.function.BiConsumer",
    "java.util.function.Predicate",
    "java.util.stream.Stream",
];

/// Simple names of [`GENERIC_TYPES`], for a hash lookup before anything else.
static SIMPLE_NAMES: std::sync::LazyLock<FxHashSet<&'static str>> =
    std::sync::LazyLock::new(|| {
        GENERIC_TYPES
            .iter()
            .map(|f| util::last_segment(f))
            .collect()
    });

/// The generic JDK type a (non-parameterized) type name refers to.
fn generic_fqn<'t>(ctx: &Ctx<'t>, name: Node<'t>) -> Option<&'static str> {
    let text = ctx.text(name);
    let simple = util::last_segment(text);
    if !SIMPLE_NAMES.contains(simple) {
        return None;
    }
    let candidate = GENERIC_TYPES.iter().find(|f| {
        f.rsplit('.').next() == Some(simple) && (text == simple || f.ends_with(text) || text == **f)
    })?;
    let fqn = ctx.resolve_type_name(text, name)?;
    // `Entry` imported from `java.util.Map.Entry` resolves to that FQN.
    (fqn == *candidate).then_some(*candidate)
}

/// Raw uses inside a type node: the type itself or its type arguments (array types skipped).
fn raw_uses<'t>(ctx: &Ctx<'t>, ty: Node<'t>, out: &mut Vec<(Node<'t>, &'static str)>, depth: u32) {
    if depth > 16 {
        return;
    }
    match kind(ty) {
        "type_identifier" | "scoped_type_identifier" => {
            if let Some(fqn) = generic_fqn(ctx, ty) {
                out.push((ty, fqn));
            }
        }
        "generic_type" => {
            for c in named_children(ty).skip(1) {
                if kind(c) == "type_arguments" {
                    for a in named_children(c) {
                        raw_uses(ctx, a, out, depth + 1);
                    }
                }
            }
        }
        "wildcard" => {
            for c in named_children(ty) {
                raw_uses(ctx, c, out, depth + 1);
            }
        }
        _ => {}
    }
}

/// Whether a declaration enclosing `node` carries `@SuppressWarnings("rawtypes")`.
fn suppressed<'t>(ctx: &Ctx<'t>, node: Node<'t>) -> bool {
    let src = ctx.source();
    let check = |decl: Node<'t>| {
        syntax::annotations(decl).any(|a| {
            syntax::annotation_name(a, src) == "SuppressWarnings"
                && ctx.text(a).contains("rawtypes")
        })
    };
    check(node)
        || ctx.ancestors(node).any(|a| {
            matches!(
                kind(a),
                "method_declaration"
                    | "constructor_declaration"
                    | "class_declaration"
                    | "interface_declaration"
                    | "enum_declaration"
                    | "record_declaration"
                    | "field_declaration"
                    | "local_variable_declaration"
            ) && check(a)
        })
}

impl<'t> JavaRule<'t> for RawType {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "field_declaration",
            "local_variable_declaration",
            "formal_parameter",
            "method_declaration",
            "object_creation_expression",
            "field_access",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let ty = match kind(node) {
            "field_access" => {
                // The field is the access's last token: test the text before reading the tree.
                let text = ctx.text(node);
                if !(text.ends_with("EMPTY_LIST")
                    || text.ends_with("EMPTY_SET")
                    || text.ends_with("EMPTY_MAP"))
                {
                    return;
                }
                let Some(field) = node.field(Field::Field) else {
                    return;
                };
                let name = ctx.text(field);
                if !matches!(name, "EMPTY_LIST" | "EMPTY_SET" | "EMPTY_MAP") {
                    return;
                }
                let Some(obj) = node.field(Field::Object) else {
                    return;
                };
                if !util::names_type(ctx, obj, "java.util.Collections") || suppressed(ctx, node) {
                    return;
                }
                let replacement = match name {
                    "EMPTY_LIST" => "emptyList()",
                    "EMPTY_SET" => "emptySet()",
                    _ => "emptyMap()",
                };
                ctx.emit(
                    node,
                    format!("Use `Collections.{replacement}` instead of the untyped `{name}`"),
                );
                return;
            }
            _ => node.field(Field::Type),
        };
        let Some(ty) = ty else {
            return;
        };
        let text = ctx.text(ty);
        // Cheap test: primitive types, `var`.
        if text == "var" || !text.starts_with(|c: char| c.is_ascii_uppercase() || c == 'j') {
            return;
        }
        let mut uses = Vec::new();
        raw_uses(ctx, ty, &mut uses, 0);
        if uses.is_empty() {
            return;
        }
        // The signature of an overriding method follows the overridden one; lambda
        // parameters follow the functional interface.
        let signature_owner = match kind(node) {
            "formal_parameter" => ctx.parent(node).and_then(|p| ctx.parent(p)),
            "method_declaration" => Some(node),
            _ => None,
        };
        if signature_owner.is_some_and(|m| {
            kind(m) == "lambda_expression"
                || (kind(m) == "method_declaration"
                    && syntax::has_annotation(m, ctx.source(), "Override"))
        }) {
            return;
        }
        if suppressed(ctx, node) {
            return;
        }
        // `ArgumentCaptor<List> c = ArgumentCaptor.forClass(List.class)`: a type argument fed
        // by a class literal can only be the raw type.
        let decl_text = ctx.text(node);
        for (at, fqn) in uses {
            let simple = util::last_segment(fqn);
            if at.id() != ty.id() && decl_text.contains(&format!("{}.class", ctx.text(at))) {
                continue;
            }
            ctx.emit(
                at,
                format!(
                    "Add type arguments to `{simple}`: a raw type turns off the compiler's type \
                     checks"
                ),
            );
        }
    }
}
