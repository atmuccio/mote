//! `java/public-static-mutable-field` — `public static` fields that are not `final`.

use super::util::{is_accessible_member, Mods};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Medium,
    tags: &["design", "encapsulation", "multi-threading"],
    remediation_minutes: 10,
    description: r#"Detects global state that any code can change: `public static` fields that
are not `final`, `public static final` fields (and interface constants) whose value is still
mutable — a non-empty array, or a mutable JDK collection created in place (`new ArrayList<>()`,
`new HashMap<>()`, `Arrays.asList(..)`) — and public fields of enums that are not `final` (an
enum constant is a single instance shared by the whole program).

**Why it matters.** Such a field is global mutable state: any code anywhere can replace its
value at any time, without synchronization or validation. Bugs caused by it are hard to trace,
the value is not safely published between threads, and tests influence each other through it.

`final` only fixes the reference: `NAMES[0] = null` or `CODES.clear()` still changes what every
other user of the constant sees.

**How to fix.** Make the field `final` if it is a constant; otherwise make it `private` and,
if outside access is really needed, expose it through methods that can validate and
synchronize. For arrays and collections, publish an unmodifiable view or copy (`List.of(..)`,
`Collections.unmodifiableMap(..)`), or keep the array private and return a copy.

```java
// Noncompliant
public static int timeoutMillis = 5_000;

// Compliant
public static final int DEFAULT_TIMEOUT_MILLIS = 5_000;
private static volatile int timeoutMillis = DEFAULT_TIMEOUT_MILLIS;
public static void setTimeoutMillis(int t) { timeoutMillis = t; }

// Noncompliant
public static final String[] LEVELS = {"low", "high"};
// Compliant
public static final List<String> LEVELS = List.of("low", "high");
```

Not reported: annotated fields (some frameworks, such as JUnit 4's `@ClassRule`, require
`public static` fields), fields of private, local, or anonymous classes, which are not
visible outside the file anyway, empty arrays (`{}`, `new String[0]`: nothing to change),
and collections whose initializer is not a known mutable JDK collection (`List.of(..)`,
`Collections.unmodifiableList(..)`, a factory method of another class)."#,
    ..RuleMeta::new(
        "java/public-static-mutable-field",
        "java",
        "Mutable public static field",
    )
};

pub struct PublicStaticMutableField;

/// Mutable collection classes of the JDK.
const MUTABLE_COLLECTIONS: &[&str] = &[
    "java.util.ArrayList",
    "java.util.LinkedList",
    "java.util.Vector",
    "java.util.Stack",
    "java.util.ArrayDeque",
    "java.util.PriorityQueue",
    "java.util.HashMap",
    "java.util.LinkedHashMap",
    "java.util.TreeMap",
    "java.util.Hashtable",
    "java.util.IdentityHashMap",
    "java.util.WeakHashMap",
    "java.util.EnumMap",
    "java.util.Properties",
    "java.util.HashSet",
    "java.util.LinkedHashSet",
    "java.util.TreeSet",
    "java.util.concurrent.ConcurrentHashMap",
    "java.util.concurrent.ConcurrentSkipListMap",
    "java.util.concurrent.ConcurrentSkipListSet",
    "java.util.concurrent.CopyOnWriteArrayList",
    "java.util.concurrent.CopyOnWriteArraySet",
    "java.util.concurrent.ConcurrentLinkedQueue",
    "java.util.concurrent.ConcurrentLinkedDeque",
    "java.util.concurrent.LinkedBlockingQueue",
    "java.util.concurrent.ArrayBlockingQueue",
];

/// What makes a `final` value mutable: `Some("array")` for a non-empty array, the class name
/// for a mutable JDK collection created in place.
fn mutable_value<'t>(ctx: &Ctx<'t>, declarator: Node<'t>, ty: Option<Node<'t>>) -> Option<String> {
    let value = syntax::strip_parens(declarator.field(Field::Value)?);
    let is_array = ty.is_some_and(|t| kind(t) == "array_type")
        || declarator.field(Field::Dimensions).is_some();
    if is_array {
        let empty = match kind(value) {
            "array_initializer" => named_children(value).next().is_none(),
            "array_creation_expression" => {
                // `new T[0]` / `new T[] {}`
                match value.field(Field::Value) {
                    Some(init) => named_children(init).next().is_none(),
                    None => named_children(value)
                        .find(|c| kind(*c) == "dimensions_expr")
                        .and_then(syntax::first_named_child)
                        .is_some_and(|d| ctx.text(d) == "0"),
                }
            }
            _ => return None,
        };
        return (!empty).then(|| "array".to_string());
    }
    match kind(value) {
        "object_creation_expression" => {
            let t = ctx.type_of(value)?;
            let fqn = t.fqn()?;
            MUTABLE_COLLECTIONS
                .contains(&fqn)
                .then(|| t.simple_name().unwrap_or(fqn).to_string())
        }
        "method_invocation" => ctx
            .is_call_to(value, "java.util.Arrays", "asList")
            .then(|| "Arrays.asList list".to_string()),
        _ => None,
    }
}

impl PublicStaticMutableField {
    /// `public static final` fields and interface constants whose value can still be changed.
    fn check_final_value<'t>(&self, node: Node<'t>, ctx: &Ctx<'t>) {
        let ty = node.field(Field::Type);
        let mut accessible = None;
        for d in syntax::declarators(node) {
            let Some(what) = mutable_value(ctx, d, ty) else {
                continue;
            };
            if !*accessible.get_or_insert_with(|| is_accessible_member(node, ctx)) {
                return;
            }
            let Some(name) = d.field(Field::Name) else {
                continue;
            };
            let name = ctx.text(name);
            let msg = if what == "array" {
                format!(
                    "`{name}` is final but its array elements can still be changed: publish an \
                     unmodifiable List or keep the array private"
                )
            } else {
                format!(
                    "`{name}` is final but its {what} can still be modified: publish an \
                     unmodifiable view (List.of, Collections.unmodifiable…) or keep it private"
                )
            };
            ctx.emit(d.field(Field::Name).unwrap_or(d), msg);
        }
    }
}

impl<'t> JavaRule<'t> for PublicStaticMutableField {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["field_declaration", "constant_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let mods = Mods::of(node);
        if mods.annotated() {
            return;
        }
        let constant = kind(node) == "constant_declaration";
        let is_public = mods.has("public") || constant;
        let is_final = mods.has("final") || constant;
        let is_static = mods.has("static") || constant;
        if !is_public {
            return;
        }
        if is_static && is_final {
            self.check_final_value(node, ctx);
            return;
        }
        let enum_field = !is_static
            && !is_final
            && ctx
                .parent(node)
                .is_some_and(|p| kind(p) == "enum_body_declarations");
        if enum_field {
            if !is_accessible_member(node, ctx) {
                return;
            }
            for d in syntax::declarators(node) {
                if let Some(name) = d.field(Field::Name) {
                    ctx.emit(
                        name,
                        format!(
                            "Make enum field `{}` final: every enum constant is shared by the \
                             whole program, so this field is global mutable state",
                            ctx.text(name)
                        ),
                    );
                }
            }
            return;
        }
        if !is_static || is_final || !is_accessible_member(node, ctx) {
            return;
        }
        for d in syntax::declarators(node) {
            if let Some(name) = d.field(Field::Name) {
                ctx.emit(
                    name,
                    format!(
                        "Make `{}` final, or make it private and control access through methods",
                        ctx.text(name)
                    ),
                );
            }
        }
    }
}
