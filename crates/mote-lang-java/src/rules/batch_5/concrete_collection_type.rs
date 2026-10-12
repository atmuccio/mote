//! `java/concrete-collection-type` — fields, parameters, and return types declared as
//! `ArrayList`, `HashMap`, … instead of an interface.

use super::util::{self, Mods, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["design", "collections"],
    remediation_minutes: 5,
    description: r#"Detects fields, method parameters, and method return types declared with a
concrete collection class — `ArrayList`, `LinkedList`, `Vector`, `HashMap`, `LinkedHashMap`,
`TreeMap`, `Hashtable`, `HashSet`, `LinkedHashSet`, `TreeSet`, and from
`java.util.concurrent` / `java.util` also `ConcurrentHashMap`, `CopyOnWriteArrayList`,
`CopyOnWriteArraySet`, `ArrayDeque`, `PriorityQueue`, `ConcurrentLinkedQueue`,
`ConcurrentLinkedDeque`, `LinkedBlockingQueue`, `ArrayBlockingQueue`, `LinkedBlockingDeque` and
`PriorityBlockingQueue` — rather than an interface such as `List`, `Map`, `Set`,
`ConcurrentMap`, `Queue`, `Deque`, or `BlockingQueue`.

**Why it matters.** A concrete type in a signature ties every caller to one implementation:
switching to another list or map, returning an unmodifiable view, or passing a
`List.of(..)` now needs changes elsewhere. Declaring the interface keeps the choice of
implementation a local detail.

**How to fix.** Declare the interface (`List`, `Map`, `Set`, or `SortedMap` / `NavigableMap` /
`Deque` when that behaviour is needed) and keep the concrete class only in the `new`
expression.

```java
// Noncompliant
private ArrayList<Order> orders = new ArrayList<>();
public HashMap<String, Integer> totals(ArrayList<Order> input) { ... }

// Compliant
private List<Order> orders = new ArrayList<>();
public Map<String, Integer> totals(List<Order> input) { ... }
```

Local variables are not reported. Also not reported: variables on which a method specific to
the implementation is called (`ensureCapacity`, `addFirst` on a list, `firstKey`,
`elementAt`, `mappingCount`, `addIfAbsent`, …; queue and deque methods are fine when the
suggested interface is a `Queue` or a `Deque`); classes whose concrete type is the point
(`IdentityHashMap`, `EnumSet`, `Stack`, `DelayQueue`);
`@Override` methods and methods of anonymous classes, whose signatures are dictated by a
supertype; annotated declarations; and fields of `Serializable` classes, where a concrete
serializable type is sometimes chosen on purpose."#,
    ..RuleMeta::new(
        "java/concrete-collection-type",
        "java",
        "Concrete collection type in a declaration",
    )
};

/// Queue methods: part of the `Queue` interface.
const QUEUE_METHODS: &[&str] = &["offer", "peek", "poll", "element"];

/// Deque methods: part of the `Deque` interface.
const DEQUE_METHODS: &[&str] = &[
    "offer",
    "peek",
    "poll",
    "element",
    "addFirst",
    "addLast",
    "getFirst",
    "getLast",
    "removeFirst",
    "removeLast",
    "offerFirst",
    "offerLast",
    "peekFirst",
    "peekLast",
    "pollFirst",
    "pollLast",
    "push",
    "pop",
    "descendingIterator",
    "removeFirstOccurrence",
    "removeLastOccurrence",
    "reversed",
];

/// A concrete collection class this rule knows.
struct Concrete {
    simple: &'static str,
    package: &'static str,
    /// The interface to suggest.
    iface: &'static str,
    /// Methods of [`IMPL_METHODS`] that the suggested interface declares as well.
    interface_methods: &'static [&'static str],
}

const CONCRETE: &[Concrete] = &[
    Concrete {
        simple: "ArrayList",
        package: "java.util",
        iface: "List",
        interface_methods: &[],
    },
    Concrete {
        simple: "LinkedList",
        package: "java.util",
        iface: "List",
        interface_methods: &[],
    },
    Concrete {
        simple: "Vector",
        package: "java.util",
        iface: "List",
        interface_methods: &[],
    },
    Concrete {
        simple: "HashMap",
        package: "java.util",
        iface: "Map",
        interface_methods: &[],
    },
    Concrete {
        simple: "LinkedHashMap",
        package: "java.util",
        iface: "Map",
        interface_methods: &[],
    },
    Concrete {
        simple: "TreeMap",
        package: "java.util",
        iface: "Map",
        interface_methods: &[],
    },
    Concrete {
        simple: "Hashtable",
        package: "java.util",
        iface: "Map",
        interface_methods: &[],
    },
    Concrete {
        simple: "HashSet",
        package: "java.util",
        iface: "Set",
        interface_methods: &[],
    },
    Concrete {
        simple: "LinkedHashSet",
        package: "java.util",
        iface: "Set",
        interface_methods: &[],
    },
    Concrete {
        simple: "TreeSet",
        package: "java.util",
        iface: "Set",
        interface_methods: &[],
    },
    Concrete {
        simple: "ArrayDeque",
        package: "java.util",
        iface: "Deque",
        interface_methods: DEQUE_METHODS,
    },
    Concrete {
        simple: "PriorityQueue",
        package: "java.util",
        iface: "Queue",
        interface_methods: QUEUE_METHODS,
    },
    Concrete {
        simple: "ConcurrentHashMap",
        package: "java.util.concurrent",
        iface: "ConcurrentMap",
        interface_methods: &[],
    },
    Concrete {
        simple: "CopyOnWriteArrayList",
        package: "java.util.concurrent",
        iface: "List",
        interface_methods: &[],
    },
    Concrete {
        simple: "CopyOnWriteArraySet",
        package: "java.util.concurrent",
        iface: "Set",
        interface_methods: &[],
    },
    Concrete {
        simple: "ConcurrentLinkedQueue",
        package: "java.util.concurrent",
        iface: "Queue",
        interface_methods: QUEUE_METHODS,
    },
    Concrete {
        simple: "ConcurrentLinkedDeque",
        package: "java.util.concurrent",
        iface: "Deque",
        interface_methods: DEQUE_METHODS,
    },
    Concrete {
        simple: "LinkedBlockingQueue",
        package: "java.util.concurrent",
        iface: "BlockingQueue",
        interface_methods: QUEUE_METHODS,
    },
    Concrete {
        simple: "ArrayBlockingQueue",
        package: "java.util.concurrent",
        iface: "BlockingQueue",
        interface_methods: QUEUE_METHODS,
    },
    Concrete {
        simple: "PriorityBlockingQueue",
        package: "java.util.concurrent",
        iface: "BlockingQueue",
        interface_methods: QUEUE_METHODS,
    },
    Concrete {
        simple: "LinkedBlockingDeque",
        package: "java.util.concurrent",
        iface: "BlockingDeque",
        interface_methods: DEQUE_METHODS,
    },
];

fn concrete_class(simple: &str) -> Option<&'static Concrete> {
    CONCRETE.iter().find(|c| c.simple == simple)
}

/// Methods that only the implementation classes offer (or that callers use to rely on it).
const IMPL_METHODS: &[&str] = &[
    // ArrayList / Vector
    "ensureCapacity",
    "trimToSize",
    "clone",
    "removeRange",
    // Vector / Hashtable
    "elementAt",
    "addElement",
    "firstElement",
    "lastElement",
    "removeElement",
    "removeElementAt",
    "insertElementAt",
    "setElementAt",
    "elements",
    "setSize",
    "capacity",
    "copyInto",
    "removeAllElements",
    "keys",
    "rehash",
    // LinkedList (Deque) and sequenced collections
    "addFirst",
    "addLast",
    "getFirst",
    "getLast",
    "removeFirst",
    "removeLast",
    "offer",
    "offerFirst",
    "offerLast",
    "peek",
    "peekFirst",
    "peekLast",
    "poll",
    "pollFirst",
    "pollLast",
    "push",
    "pop",
    "element",
    "descendingIterator",
    "removeFirstOccurrence",
    "removeLastOccurrence",
    "reversed",
    "putFirst",
    "putLast",
    // TreeMap / TreeSet (SortedMap, NavigableMap, …)
    "firstKey",
    "lastKey",
    "headMap",
    "tailMap",
    "subMap",
    "firstEntry",
    "lastEntry",
    "pollFirstEntry",
    "pollLastEntry",
    "ceilingKey",
    "ceilingEntry",
    "floorKey",
    "floorEntry",
    "higherKey",
    "higherEntry",
    "lowerKey",
    "lowerEntry",
    "descendingMap",
    "descendingKeySet",
    "navigableKeySet",
    "comparator",
    "first",
    "last",
    "headSet",
    "tailSet",
    "subSet",
    "ceiling",
    "floor",
    "higher",
    "lower",
    "descendingSet",
    // LinkedHashMap
    "removeEldestEntry",
    // ConcurrentHashMap (parallel bulk operations, counts, key-set views)
    "mappingCount",
    "keySet",
    "forEach",
    "search",
    "searchKeys",
    "searchValues",
    "searchEntries",
    "reduce",
    "reduceKeys",
    "reduceValues",
    "reduceEntries",
    "reduceToInt",
    "reduceToLong",
    "reduceToDouble",
    "reduceKeysToInt",
    "reduceKeysToLong",
    "reduceKeysToDouble",
    "reduceValuesToInt",
    "reduceValuesToLong",
    "reduceValuesToDouble",
    "reduceEntriesToInt",
    "reduceEntriesToLong",
    "reduceEntriesToDouble",
    "forEachKey",
    "forEachValue",
    "forEachEntry",
    // CopyOnWriteArrayList / CopyOnWriteArraySet
    "addIfAbsent",
    "addAllAbsent",
    // Blocking queues (capacity) and PriorityQueue (ordering)
    "remainingCapacity",
];

/// Methods of [`IMPL_METHODS`] that only count for some classes: `ConcurrentHashMap`'s
/// parallel `forEach(parallelism, …)` and value-backed `keySet(value)` take one argument more
/// than the `Map` methods of the same name.
fn is_impl_method(c: &Concrete, method: &str, has_extra_argument: bool) -> bool {
    if c.interface_methods.contains(&method) {
        return false;
    }
    match method {
        "forEach" | "keySet" => c.simple == "ConcurrentHashMap" && has_extra_argument,
        "remainingCapacity" => c.iface == "Queue",
        // The legacy `contains(value)` of `Hashtable` and `ConcurrentHashMap`.
        "contains" => matches!(c.simple, "Hashtable" | "ConcurrentHashMap"),
        _ => IMPL_METHODS.contains(&method),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Field,
    Parameter,
    Return,
}

struct Candidate<'t> {
    type_node: Node<'t>,
    class: &'static Concrete,
    role: Role,
    /// Variable name (empty for return types).
    name: &'t str,
    /// Where uses of the variable can be.
    scope: (usize, usize),
}

#[derive(Default)]
pub struct ConcreteCollectionType<'t> {
    candidates: Vec<Candidate<'t>>,
}

impl<'t> ConcreteCollectionType<'t> {
    /// The concrete class when `ty` provably names one of the JDK collection classes.
    fn concrete(ctx: &Ctx<'t>, ty: Node<'t>) -> Option<&'static Concrete> {
        let base = util::type_base(ty);
        let text = ctx.text(base);
        let simple = text.rsplit('.').next().unwrap_or(text);
        let class = concrete_class(simple)?;
        let fqn = util::type_fqn(ctx, ty)?;
        let (package, name) = fqn.rsplit_once('.')?;
        (package == class.package && name == simple).then_some(class)
    }

    fn params(&mut self, ctx: &Ctx<'t>, decl: Node<'t>, scope: (usize, usize)) {
        for p in syntax::parameters(decl) {
            if kind(p) != "formal_parameter" || Mods::of(p).annotated() {
                continue;
            }
            let (Some(ty), Some(name)) = (util::fld(p, F.ty), util::fld(p, F.name)) else {
                continue;
            };
            if let Some(class) = Self::concrete(ctx, ty) {
                self.candidates.push(Candidate {
                    type_node: util::type_base(ty),
                    class,
                    role: Role::Parameter,
                    name: ctx.text(name),
                    scope,
                });
            }
        }
    }

    /// Whether an implementation-specific method is called on `name` within the scope.
    fn uses_impl_methods(
        ctx: &Ctx<'_>,
        class: &Concrete,
        name: &str,
        scope: (usize, usize),
    ) -> bool {
        let src = ctx.source();
        let b = src.as_bytes();
        util::source_words(ctx, name, scope.0, scope.1).any(|at| {
            let mut i = at + name.len();
            while b.get(i).is_some_and(u8::is_ascii_whitespace) {
                i += 1;
            }
            if b.get(i) != Some(&b'.') {
                return false;
            }
            i += 1;
            while b.get(i).is_some_and(u8::is_ascii_whitespace) {
                i += 1;
            }
            let start = i;
            while b
                .get(i)
                .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_')
            {
                i += 1;
            }
            let Some(m) = src.get(start..i) else {
                return false;
            };
            // `forEach(parallelism, action)`: a comma at the top level of the arguments.
            let has_extra_argument = || {
                let mut j = i;
                while b.get(j).is_some_and(u8::is_ascii_whitespace) {
                    j += 1;
                }
                if b.get(j) != Some(&b'(') {
                    return false;
                }
                let mut depth = 0i32;
                for &c in b.get(j..(j + 2000).min(b.len())).unwrap_or(&[]) {
                    match c {
                        b'(' | b'[' | b'{' | b'<' => depth += 1,
                        b')' | b']' | b'}' | b'>' => {
                            depth -= 1;
                            if depth == 0 {
                                return false;
                            }
                        }
                        b',' if depth == 1 => return true,
                        _ => {}
                    }
                }
                // Unbalanced or too long: treat as implementation-specific.
                true
            };
            is_impl_method(
                class,
                m,
                matches!(m, "forEach" | "keySet") && has_extra_argument(),
            )
        })
    }
}

/// Methods whose signature a supertype dictates: `@Override`, or members of anonymous classes
/// and enum constant bodies.
fn signature_is_dictated<'t>(ctx: &Ctx<'t>, method: Node<'t>) -> bool {
    if Mods::of(method).has_annotation(ctx.source(), "Override") {
        return true;
    }
    ctx.enclosing_class(method)
        .is_some_and(|c| matches!(kind(c), "object_creation_expression" | "enum_constant"))
}

/// Whether `text` may name one of the concrete classes (a superset test).
fn mentions_concrete_class(text: &str) -> bool {
    text.contains("List")
        || text.contains("Map")
        || text.contains("Set")
        || text.contains("Vector")
        || text.contains("Hashtable")
        || text.contains("Queue")
        || text.contains("Deque")
}

fn is_serializable_class(ctx: &Ctx<'_>, class: Node<'_>) -> bool {
    named_children(class)
        .filter(|c| kind(*c) == "super_interfaces")
        .any(|c| {
            let t = ctx.text(c);
            t.contains("Serializable") || t.contains("Externalizable")
        })
}

impl<'t> JavaRule<'t> for ConcreteCollectionType<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "field_declaration",
            "method_declaration",
            "constructor_declaration",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        match kind(node) {
            "field_declaration" => {
                let Some(ty) = util::fld(node, F.ty) else {
                    return;
                };
                if !matches!(
                    kind(ty),
                    "generic_type" | "type_identifier" | "scoped_type_identifier"
                ) {
                    return;
                }
                let Some(class) = Self::concrete(ctx, ty) else {
                    return;
                };
                if Mods::of(node).annotated() {
                    return;
                }
                let owner = ctx.enclosing_class(node);
                if owner.is_some_and(|c| is_serializable_class(ctx, c)) {
                    return;
                }
                let scope = (0, ctx.source().len());
                for d in syntax::declarators(node) {
                    if util::fld(d, F.dimensions).is_some() {
                        continue;
                    }
                    let Some(name) = util::fld(d, F.name) else {
                        continue;
                    };
                    self.candidates.push(Candidate {
                        type_node: util::type_base(ty),
                        class,
                        role: Role::Field,
                        name: ctx.text(name),
                        scope,
                    });
                }
            }
            "method_declaration" | "constructor_declaration" => {
                // Cheap text test first: the signature (return type and parameters) must
                // mention one of the concrete classes.
                let sig_end = util::fld(node, F.parameters).map_or(0, |p| p.end_byte());
                let sig = ctx
                    .source()
                    .get(node.start_byte()..sig_end.max(node.start_byte()))
                    .unwrap_or("");
                if !mentions_concrete_class(sig) {
                    return;
                }
                if kind(node) == "method_declaration" && signature_is_dictated(ctx, node) {
                    return;
                }
                if kind(node) == "method_declaration" {
                    if let Some(ty) = util::fld(node, F.ty) {
                        if matches!(
                            kind(ty),
                            "generic_type" | "type_identifier" | "scoped_type_identifier"
                        ) && util::fld(node, F.dimensions).is_none()
                        {
                            if let Some(class) = Self::concrete(ctx, ty) {
                                self.candidates.push(Candidate {
                                    type_node: util::type_base(ty),
                                    class,
                                    role: Role::Return,
                                    name: "",
                                    scope: (0, 0),
                                });
                            }
                        }
                    }
                }
                let Some(body) = util::fld(node, F.body) else {
                    return;
                };
                self.params(ctx, node, (body.start_byte(), body.end_byte()));
            }
            _ => {}
        }
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        for c in &self.candidates {
            if c.role != Role::Return && Self::uses_impl_methods(ctx, c.class, c.name, c.scope) {
                continue;
            }
            let (concrete, iface, name) = (c.class.simple, c.class.iface, c.name);
            let msg = match c.role {
                Role::Field => {
                    format!("Declare field `{name}` as `{iface}` instead of the class `{concrete}`")
                }
                Role::Parameter => format!(
                    "Declare parameter `{name}` as `{iface}` instead of the class `{concrete}`"
                ),
                Role::Return => {
                    format!("Return `{iface}` instead of the concrete class `{concrete}`")
                }
            };
            ctx.emit(c.type_node, msg);
        }
    }
}
