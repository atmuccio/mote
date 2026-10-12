//! `java/volatile-mutable-contents` — a `volatile` array or collection whose contents are
//! modified in place.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["concurrency"],
    remediation_minutes: 15,
    description: r#"Detects a `volatile` field holding an array or a non-thread-safe JDK
collection (`ArrayList`, `HashMap`, …) whose elements the class modifies in place —
`values[i] = x`, `items.add(x)`, `cache.put(k, v)` — outside any `synchronized` code.

**Why it matters.** `volatile` only applies to the reference: reading the field sees the
latest array or collection object, but writes to its elements or entries get no visibility
or atomicity guarantee. Other threads can see stale elements, and concurrent `add` / `put`
calls corrupt a `HashMap` or `ArrayList`. The keyword gives a false impression that the
contents are safe to share.

**How to fix.** Use a thread-safe structure (`AtomicIntegerArray`, `ConcurrentHashMap`,
`CopyOnWriteArrayList`), guard every access with a lock, or treat the contents as immutable
and publish a new copy through the volatile field on each change.

```java
// Noncompliant
private volatile Map<String, Session> sessions = new HashMap<>();
void open(String id, Session s) { sessions.put(id, s); }

// Compliant
private final Map<String, Session> sessions = new ConcurrentHashMap<>();
void open(String id, Session s) { sessions.put(id, s); }
```

Not reported: copy-on-write use (the field is only ever replaced by a new object), fields
also assigned something other than a new JDK array or collection (its thread safety is then
unknown), and changes made inside `synchronized` code. Off by default: it needs the whole
class's accesses to be judged."#,
    ..RuleMeta::new(
        "java/volatile-mutable-contents",
        "java",
        "Volatile array or collection modified in place",
    )
};

/// JDK collection classes that are not thread-safe.
const UNSAFE_COLLECTIONS: &[&str] = &[
    "ArrayList",
    "LinkedList",
    "HashMap",
    "LinkedHashMap",
    "TreeMap",
    "HashSet",
    "LinkedHashSet",
    "TreeSet",
    "ArrayDeque",
    "PriorityQueue",
    "IdentityHashMap",
    "WeakHashMap",
    "EnumMap",
];

/// Collection methods that modify the contents.
const MUTATORS: &[&str] = &[
    "add",
    "addAll",
    "put",
    "putAll",
    "remove",
    "removeAll",
    "removeIf",
    "retainAll",
    "clear",
    "set",
    "sort",
    "replaceAll",
    "putIfAbsent",
    "computeIfAbsent",
    "computeIfPresent",
    "compute",
    "merge",
    "offer",
    "push",
    "pop",
    "poll",
    "addFirst",
    "addLast",
    "removeFirst",
    "removeLast",
];

struct VolField<'t> {
    declarator: Node<'t>,
    name: &'t str,
    is_array: bool,
}

#[derive(Default)]
pub struct VolatileMutableContents<'t> {
    fields: Vec<VolField<'t>>,
    /// Writes to elements: (the array / receiver expression, the reported node).
    writes: Vec<(Node<'t>, Node<'t>)>,
    /// Assignments `f = value` / `this.f = value`: (target, value).
    assigns: Vec<(Node<'t>, Node<'t>)>,
    gate: util::Gate,
}

impl<'t> JavaRule<'t> for VolatileMutableContents<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "field_declaration",
            "assignment_expression",
            "update_expression",
            "method_invocation",
        ]
    }

    fn call_names(&self) -> &'static [&'static str] {
        MUTATORS
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !self.gate.open(ctx, "volatile") {
            return;
        }
        match kind(node) {
            "field_declaration" => self.collect_field(node, ctx),
            "assignment_expression" => {
                let (Some(l), Some(r)) = (node.field(Field::Left), node.field(Field::Right)) else {
                    return;
                };
                if kind(l) == "array_access" {
                    if let Some(arr) = l.field(Field::Array) {
                        self.writes.push((arr, l));
                    }
                } else if syntax::operator_str(node) == "=" {
                    self.assigns.push((l, r));
                }
            }
            "update_expression" => {
                if let Some(target) = syntax::first_named_child(node) {
                    if kind(target) == "array_access" {
                        if let Some(arr) = target.field(Field::Array) {
                            self.writes.push((arr, target));
                        }
                    }
                }
            }
            _ => {
                let Some(name) = ctx.call_name(node) else {
                    return;
                };
                if !MUTATORS.contains(&name) {
                    return;
                }
                if let Some(obj) = node.field(Field::Object) {
                    self.writes.push((obj, node));
                }
            }
        }
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        for f in &self.fields {
            // Every assignment must store a fresh JDK array / unsafe collection.
            let fresh_only = self
                .assigns
                .iter()
                .filter(|(target, _)| refers_to(ctx, *target, f))
                .all(|(_, value)| is_fresh(ctx, *value, f.is_array));
            if !fresh_only {
                continue;
            }
            let Some(&(_, at)) = self.writes.iter().find(|(target, at)| {
                refers_to(ctx, *target, f)
                    && (f.is_array == (kind(*at) == "array_access"))
                    && unsynchronized_method_code(ctx, *at)
            }) else {
                continue;
            };
            let what = if f.is_array { "array" } else { "collection" };
            let Some(name_node) = f.declarator.field(Field::Name) else {
                continue;
            };
            ctx.emit(
                name_node,
                format!("Do not rely on `volatile` for the contents of `{}`: it only publishes the {what} reference, not the changes made to its elements in place", f.name),
            )
            .secondary(at, "modified in place here");
        }
    }
}

impl<'t> VolatileMutableContents<'t> {
    fn collect_field(&mut self, decl: Node<'t>, ctx: &Ctx<'t>) {
        if !syntax::has_modifier(decl, "volatile") {
            return;
        }
        let Some(ty) = decl.field(Field::Type) else {
            return;
        };
        for d in syntax::declarators(decl) {
            let is_array = kind(ty) == "array_type" || d.field(Field::Dimensions).is_some();
            if !is_array {
                let Some(t) = ctx.resolve_type(ty) else {
                    continue;
                };
                let collection = ctx.is_subtype_of(&t, "java.util.Collection")
                    || ctx.is_subtype_of(&t, "java.util.Map");
                if !collection
                    || !t.fqn().is_some_and(|f| {
                        f.starts_with("java.util.") && !f.starts_with("java.util.concurrent")
                    })
                {
                    continue;
                }
            }
            if let Some(v) = d.field(Field::Value) {
                if !is_fresh(ctx, v, is_array) {
                    continue;
                }
            }
            let Some(name) = d.field(Field::Name) else {
                continue;
            };
            self.fields.push(VolField {
                declarator: d,
                name: ctx.text(name),
                is_array,
            });
        }
    }
}

/// Whether `value` is a new array (for arrays) or a new non-thread-safe JDK collection.
fn is_fresh<'t>(ctx: &Ctx<'t>, value: Node<'t>, array: bool) -> bool {
    let v = syntax::strip_parens(value);
    if array {
        return matches!(kind(v), "array_creation_expression" | "array_initializer");
    }
    if kind(v) != "object_creation_expression" || syntax::child_of_kind(v, "class_body").is_some() {
        return false;
    }
    let Some(ty) = v.field(Field::Type) else {
        return false;
    };
    let base = if kind(ty) == "generic_type" {
        syntax::first_named_child(ty).unwrap_or(ty)
    } else {
        ty
    };
    let simple = util::last_segment(ctx.text(base));
    UNSAFE_COLLECTIONS.contains(&simple)
        && ctx
            .resolve_type(base)
            .and_then(|t| t.fqn().map(str::to_string))
            .is_some_and(|f| f == format!("java.util.{simple}"))
}

/// Whether `expr` (`f` or `this.f`) denotes the field.
fn refers_to<'t>(ctx: &Ctx<'t>, expr: Node<'t>, f: &VolField<'t>) -> bool {
    let e = syntax::strip_parens(expr);
    let ident = match kind(e) {
        "identifier" => e,
        "field_access" => {
            let (Some(o), Some(fl)) = (e.field(Field::Object), e.field(Field::Field)) else {
                return false;
            };
            if kind(o) != "this" {
                return false;
            }
            fl
        }
        _ => return false,
    };
    if ctx.text(ident) != f.name {
        return false;
    }
    ctx.declaration_of(ident)
        .is_some_and(|d| d.decl.id() == f.declarator.id())
}

/// Whether a write at `node` may race: it is in a method (not a constructor or initializer,
/// which run before the object is shared) and not inside `synchronized` code. Lambdas and
/// nested classes count as unsynchronized method code.
fn unsynchronized_method_code<'t>(ctx: &Ctx<'t>, node: Node<'t>) -> bool {
    let mut deferred = false;
    for a in ctx.ancestors(node) {
        match kind(a) {
            "synchronized_statement" => return false,
            "method_declaration" => return !syntax::has_modifier(a, "synchronized"),
            "lambda_expression" | "class_body" => deferred = true,
            "constructor_declaration" | "static_initializer" | "field_declaration" => {
                return deferred;
            }
            "block" if ctx.parent(a).is_some_and(|p| kind(p) == "class_body") => {
                return deferred;
            }
            _ => {}
        }
    }
    false
}
