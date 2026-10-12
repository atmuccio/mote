//! `java/incompatible-collection-lookup` — `Set<Long>.contains(intValue)` and friends.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["collections", "api-misuse"],
    remediation_minutes: 5,
    description: r#"Detects collection and map lookups whose argument can never be equal to an
element or key: `Collection.contains/remove`, `List.indexOf/lastIndexOf`, and
`Map.get/containsKey/containsValue/remove/getOrDefault` called with a value of a type
unrelated to the declared element, key, or value type.

**Why it matters.** These methods take `Object`, so the compiler accepts any argument. A
`Set<Long>` never contains an `Integer`, and a `Map<String, User>` never has a key of type
`UUID` — the lookup always misses: `contains` is always `false`, `get` always `null`, and
`remove` silently removes nothing. The mismatch usually comes from autoboxing (`int` vs
`Long`) or from passing the wrong variable.

**How to fix.** Convert the argument to the element/key type (`set.contains((long) id)`,
`map.get(id.toString())`), or pass the variable that was meant.

```java
// Noncompliant
Set<Long> ids = loadIds();
int id = request.id();
if (ids.contains(id)) { … }   // Integer vs Long: always false

// Compliant
if (ids.contains((long) id)) { … }
```

Reported only when both types are known and one of them is a final JDK class (`String`,
the boxed primitives, `UUID`, `java.time` values, …) that is provably unrelated to the
other, so no object can ever be both. The receiver must be a variable, parameter, or field
declared in this file with a parameterized collection or map type whose element, key, or
value type is such a final class; the argument must be a literal, a cast, or a variable
(call results and `var` receivers are not checked). `List.remove(int)` (removal by index)
is not a lookup and is not considered."#,
    ..RuleMeta::new(
        "java/incompatible-collection-lookup",
        "java",
        "Collection lookup with an incompatible type",
    )
};

/// Lookups are recorded during the walk; with the file's index of declared types
/// ([`Ctx::declared_types`]),
/// `finish` compares the declared type argument with the argument's apparent type *textually*
/// and asks the resolver only about the rare mismatches (`Set<Long>` vs `int`), so the common
/// `map.get(key)` never costs a type resolution.
#[derive(Default)]
pub struct IncompatibleCollectionLookup<'t> {
    candidates: Vec<Candidate<'t>>,
}

/// A lookup call waiting for `finish`.
#[derive(Clone, Copy)]
struct Candidate<'t> {
    name_node: Node<'t>,
    recv: Node<'t>,
    arg: Node<'t>,
    recv_name: &'t str,
    map_method: bool,
    slot: Slot,
}

/// Which type argument of the receiver the lookup argument is compared with.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Slot {
    Element,
    Key,
    Value,
}

/// Final JDK value classes (simple names) worth checking as element/key/value types.
const FINAL_VALUE_TYPES: &[&str] = &[
    "String",
    "Integer",
    "Long",
    "Short",
    "Byte",
    "Character",
    "Boolean",
    "Double",
    "Float",
    "UUID",
    "URI",
    "LocalDate",
    "LocalDateTime",
    "LocalTime",
    "Instant",
    "ZonedDateTime",
    "OffsetDateTime",
    "Duration",
    "Period",
];

fn is_map_name(base: &str) -> bool {
    base.ends_with("Map") || base == "Hashtable"
}

fn is_collection_name(base: &str) -> bool {
    [
        "Set",
        "List",
        "Collection",
        "Queue",
        "Deque",
        "Vector",
        "Stack",
    ]
    .iter()
    .any(|s| base.ends_with(s))
}

/// Wrapper name for a primitive type name; other names unchanged.
fn boxed_name(name: &str) -> &str {
    match name {
        "int" => "Integer",
        "long" => "Long",
        "short" => "Short",
        "byte" => "Byte",
        "char" => "Character",
        "boolean" => "Boolean",
        "double" => "Double",
        "float" => "Float",
        other => other,
    }
}

/// Box a primitive; other types unchanged.
fn boxed(t: JType) -> JType {
    match t {
        JType::Prim(p) => JType::class(p.boxed_fqn()),
        other => other,
    }
}

/// Whether no value can be both an `a` and a `b`: both are known class types, and one of them
/// is a final JDK class that is provably not a subtype of the other (a final class has no
/// subclasses, so the other type's objects can only be `a` if `a` extends it).
fn disjoint(ctx: &Ctx<'_>, a: &JType, b: &JType) -> bool {
    let (Some(fa), Some(fb)) = (a.fqn(), b.fqn()) else {
        return false;
    };
    if fa == fb {
        return false;
    }
    let final_a = crate::resolve::is_final_class(fa);
    let final_b = crate::resolve::is_final_class(fb);
    (final_a && ctx.subtype(a, fb) == Some(false)) || (final_b && ctx.subtype(b, fa) == Some(false))
}

impl<'t> IncompatibleCollectionLookup<'t> {
    /// A lookup call: record it when its shape matches.
    fn collect_call(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name_node) = util::fld(node, F::Name) else {
            return;
        };
        let (map_method, slot, arity) = match ctx.text(name_node) {
            "contains" | "remove" | "indexOf" | "lastIndexOf" => (false, Slot::Element, 1),
            "get" | "containsKey" => (true, Slot::Key, 1),
            "getOrDefault" => (true, Slot::Key, 2),
            "containsValue" => (true, Slot::Value, 1),
            _ => return,
        };
        if util::arg_count(node) != arity {
            return;
        }
        let Some(recv) = util::fld(node, F::Object) else {
            return;
        };
        let Some(recv_name) = util::var_name(ctx, recv) else {
            return;
        };
        let Some(arg) = util::first_arg(node) else {
            return;
        };
        self.candidates.push(Candidate {
            name_node,
            recv,
            arg,
            recv_name,
            map_method,
            slot,
        });
    }

    /// The simple name of the declared type argument the lookup is compared with, when the
    /// receiver's declaration is unambiguous and the argument is a final JDK value type.
    fn slot_type_text(&self, ctx: &Ctx<'t>, c: &Candidate<'t>) -> Option<&'t str> {
        let name = ctx.text(c.name_node);
        let t = ctx.declared_types().type_of_name(c.recv_name)?;
        if kind(t) != "generic_type" {
            return None;
        }
        let base = util::simple_type_name(ctx, t);
        let index = if is_map_name(base) {
            if !c.map_method && name != "remove" {
                return None;
            }
            usize::from(c.slot == Slot::Value)
        } else if is_collection_name(base) {
            if c.map_method {
                return None;
            }
            0
        } else {
            return None;
        };
        let targs = named_children(t).find(|n| kind(*n) == "type_arguments")?;
        let arg_type = named_children(targs).nth(index)?;
        if !matches!(kind(arg_type), "type_identifier" | "scoped_type_identifier") {
            return None;
        }
        let text = util::simple_type_name(ctx, arg_type);
        FINAL_VALUE_TYPES.contains(&text).then_some(text)
    }

    /// The apparent (boxed) type name of a lookup argument, from its syntax or the declared-type
    /// index; `None` when it can't be told without the resolver.
    fn arg_type_text(&self, ctx: &Ctx<'t>, arg: Node<'t>) -> Option<&'t str> {
        let arg = syntax::strip_parens(arg);
        match kind(arg) {
            "string_literal" => Some("String"),
            "character_literal" => Some("Character"),
            "true" | "false" => Some("Boolean"),
            "decimal_integer_literal"
            | "hex_integer_literal"
            | "octal_integer_literal"
            | "binary_integer_literal" => Some(if ctx.text(arg).ends_with(['l', 'L']) {
                "Long"
            } else {
                "Integer"
            }),
            "decimal_floating_point_literal" => Some(if ctx.text(arg).ends_with(['f', 'F']) {
                "Float"
            } else {
                "Double"
            }),
            "cast_expression" => {
                let t = util::fld(arg, F::Type)?;
                Some(boxed_name(util::simple_type_name(ctx, t)))
            }
            "identifier" | "field_access" => {
                let t = ctx
                    .declared_types()
                    .type_of_name(util::var_name(ctx, arg)?)?;
                if kind(t) == "array_type" {
                    return None;
                }
                Some(boxed_name(util::simple_type_name(ctx, t)))
            }
            _ => None,
        }
    }

    fn check_candidate(&self, ctx: &Ctx<'t>, c: Candidate<'t>) {
        let name = ctx.text(c.name_node);
        let Some(recv_t) = ctx.type_of(c.recv) else {
            return;
        };
        if !recv_t.is_reference() || recv_t.is_string() {
            return;
        }
        // Resolve the declared element/key/value type through the receiver's supertypes.
        let expected = if c.map_method || name == "remove" {
            // `remove(Object)` exists on both; Map first.
            match ctx.as_supertype(&recv_t, "java.util.Map") {
                Some(m) => {
                    let i = usize::from(c.slot == Slot::Value);
                    m.type_arg(i).cloned()
                }
                None if !c.map_method => ctx
                    .as_supertype(&recv_t, "java.util.Collection")
                    .and_then(|t| t.type_arg(0).cloned()),
                None => None,
            }
        } else {
            ctx.as_supertype(&recv_t, "java.util.Collection")
                .and_then(|t| t.type_arg(0).cloned())
        };
        let Some(expected) = expected else {
            return;
        };
        let Some(arg_t) = ctx.type_of(c.arg) else {
            return;
        };
        // `list.remove(int)` removes by index.
        if name == "remove" && arg_t.is_primitive() && ctx.is_subtype_of(&recv_t, "java.util.List")
        {
            return;
        }
        if arg_t.is_null() || arg_t.is_array() || expected.is_array() {
            return;
        }
        let arg_t = boxed(arg_t);
        if !disjoint(ctx, &arg_t, &expected) {
            return;
        }
        let what = match c.slot {
            Slot::Element => "element",
            Slot::Key => "key",
            Slot::Value => "value",
        };
        ctx.emit(
            c.name_node,
            format!("This `{name}(..)` can never match: the argument is a `{arg_t}` but the {what} type is `{expected}`"),
        );
    }
}

impl<'t> JavaRule<'t> for IncompatibleCollectionLookup<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &[
            "contains",
            "remove",
            "indexOf",
            "lastIndexOf",
            "get",
            "containsKey",
            "getOrDefault",
            "containsValue",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        self.collect_call(node, ctx);
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        let candidates = std::mem::take(&mut self.candidates);
        // Bounded resolver work per file.
        let mut budget = 256u32;
        for c in candidates {
            let Some(slot_text) = self.slot_type_text(ctx, &c) else {
                continue;
            };
            let Some(arg_text) = self.arg_type_text(ctx, c.arg) else {
                continue;
            };
            if slot_text == arg_text || arg_text == "Object" {
                continue;
            }
            if budget == 0 {
                break;
            }
            budget -= 1;
            self.check_candidate(ctx, c);
        }
    }
}
