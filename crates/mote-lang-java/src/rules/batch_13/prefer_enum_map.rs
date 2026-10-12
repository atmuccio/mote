//! `java/prefer-enum-map` — a `HashMap`/`TreeMap`/`HashSet`/`TreeSet` keyed by an enum.

use super::util;
use crate::prelude::*;
use crate::resolve;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Incubating,
    tags: &["performance", "collections", "enum"],
    remediation_minutes: 5,
    description: r#"Detects a new `HashMap` or `TreeMap` whose keys are an enum, and a new `HashSet`
or `TreeSet` of an enum, when the enum is declared in the same file or is a JDK enum
(`TimeUnit`, `DayOfWeek`, …).

**Why it matters.** `EnumMap` and `EnumSet` are built for enum keys: an `EnumMap` is an array
indexed by the constant's ordinal and an `EnumSet` a bit mask, so they use a fraction of the
memory of a hash table, never hash or box anything, and iterate in the enum's declaration
order — the same order a `TreeMap` or `TreeSet` of an enum gives, and a stable one instead of
a `HashMap`'s unspecified order.

**How to fix.** `new EnumMap<>(Color.class)` for a map, `EnumSet.noneOf(Color.class)` for a
set. Note that they reject `null` keys.

```java
// Noncompliant
Map<Color, Integer> counts = new HashMap<>();
Set<Color> seen = new HashSet<>();

// Compliant
Map<Color, Integer> counts = new EnumMap<>(Color.class);
Set<Color> seen = EnumSet.noneOf(Color.class);
```

Not reported: enums declared in other files (not visible here), `LinkedHashMap` and
`LinkedHashSet` (their order is the insertion order), concurrent maps, a `TreeMap` or
`TreeSet` given a comparator (a custom order), copies of another collection
(`EnumSet.copyOf` rejects an empty collection that is not an `EnumSet`), and anonymous
subclasses."#,
    ..RuleMeta::new(
        "java/prefer-enum-map",
        "java",
        "Enum-keyed map or set that could be an EnumMap or EnumSet",
    )
};

#[derive(Default)]
pub struct PreferEnumMap<'t> {
    /// Names after `enum` in this file, collected on first use.
    enum_names: Option<Vec<&'t str>>,
}

/// Simple names of the JDK enums the resolver knows (`resolve::is_enum_class`).
const JDK_ENUMS: &[&str] = &[
    "TimeUnit",
    "LinkOption",
    "FileVisitOption",
    "StandardOpenOption",
    "StandardCopyOption",
    "RoundingMode",
    "ChronoUnit",
    "JDBCType",
    "RowIdLifetime",
    "ClientInfoStatus",
    "Kind",
    "NestingKind",
    "Modifier",
    "MemoryType",
];

impl<'t> JavaRule<'t> for PreferEnumMap<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["object_creation_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(ty) = node.field(Field::Type) else {
            return;
        };
        if kind(ty) != "generic_type" {
            return; // raw types: the key type is not stated
        }
        let Some(base) = syntax::first_named_child(ty) else {
            return;
        };
        let simple = util::last_segment(ctx.text(base));
        let (is_map, sorted) = match simple {
            "HashMap" => (true, false),
            "TreeMap" => (true, true),
            "HashSet" => (false, false),
            "TreeSet" => (false, true),
            _ => return,
        };
        if syntax::child_of_kind(node, "class_body").is_some() {
            return;
        }
        let Some(targs) = syntax::child_of_kind(ty, "type_arguments") else {
            return;
        };
        let key_node = match syntax::first_named_child(targs) {
            Some(k) => Some(k),
            None => diamond_key(ctx, node),
        };
        let Some(key_node) = key_node else {
            return;
        };
        // Cheap text filter before any resolver work: the key names a JDK enum, or an enum
        // declared in this file.
        if !matches!(kind(key_node), "type_identifier" | "scoped_type_identifier") {
            return;
        }
        let key_name = util::last_segment(ctx.text(key_node));
        if !JDK_ENUMS.contains(&key_name) {
            let names = self
                .enum_names
                .get_or_insert_with(|| enum_names(ctx.source()));
            if !names.contains(&key_name) {
                return;
            }
        }
        // No argument, or an initial capacity for the hash-based ones.
        let mut args = util::args(node);
        match (args.next(), args.next()) {
            (None, _) => {}
            (Some(a), None) if !sorted => {
                if !ctx
                    .type_of(a)
                    .and_then(|t| t.prim())
                    .is_some_and(|p| p.is_integral())
                {
                    return;
                }
            }
            _ => return,
        }
        let fqn = format!("java.util.{simple}");
        if ctx.resolve_type(base).as_ref().and_then(|t| t.fqn()) != Some(fqn.as_str()) {
            return;
        }
        let Some(key) = ctx.resolve_type(key_node) else {
            return;
        };
        let Some(key_fqn) = key.fqn() else {
            return;
        };
        let is_enum = resolve::is_enum_class(key_fqn) || ctx.is_subtype_of(&key, "java.lang.Enum");
        if !is_enum {
            return;
        }
        let enum_name = key_fqn.rsplit('.').next().unwrap_or(key_fqn);
        let msg = if is_map {
            format!("Use an `EnumMap` for this map keyed by the enum `{enum_name}`: `new EnumMap<>({enum_name}.class)` is smaller and faster")
        } else {
            format!("Use an `EnumSet` for this set of the enum `{enum_name}`: `EnumSet.noneOf({enum_name}.class)` is smaller and faster")
        };
        ctx.emit(ty, msg);
    }
}

/// The identifiers following the word `enum` in the source (a superset of the enums it
/// declares).
fn enum_names(src: &str) -> Vec<&str> {
    let is_ident = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(pos) = src[from..].find("enum") {
        let start = from + pos;
        from = start + 4;
        if src[..start].chars().next_back().is_some_and(is_ident) {
            continue;
        }
        let rest = &src[from..];
        let trimmed = rest.trim_start();
        if trimmed.len() == rest.len() {
            continue; // `enum` must be followed by white space
        }
        let end = trimmed
            .find(|c: char| !is_ident(c))
            .unwrap_or(trimmed.len());
        if end > 0 {
            out.push(&trimmed[..end]);
        }
    }
    out
}

/// The key type node of a diamond creation, from the declared type it initializes or is
/// assigned to.
fn diamond_key<'t>(ctx: &Ctx<'t>, node: Node<'t>) -> Option<Node<'t>> {
    let parent = ctx.parent(node)?;
    let declared = match kind(parent) {
        "variable_declarator" => {
            if parent.field(Field::Value).map(|v| v.id()) != Some(node.id()) {
                return None;
            }
            ctx.parent(parent)?.field(Field::Type)?
        }
        "assignment_expression" => {
            if parent.field(Field::Right).map(|v| v.id()) != Some(node.id()) {
                return None;
            }
            let left = parent.field(Field::Left)?;
            if kind(left) != "identifier" {
                return None;
            }
            ctx.declared_types().type_of_name(ctx.text(left))?
        }
        _ => return None,
    };
    if kind(declared) != "generic_type" {
        return None;
    }
    syntax::first_named_child(syntax::child_of_kind(declared, "type_arguments")?)
}
