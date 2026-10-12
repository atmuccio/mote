//! `java/boxing-roundtrip` — a value boxed with `valueOf` only to be unboxed right away, or
//! another temporary object that a static call or a literal avoids.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["performance", "boxing"],
    remediation_minutes: 2,
    description: r#"Detects a wrapper created with `Integer.valueOf(..)` (or `Long`, `Double`,
`Float`, `Short`, `Byte`, `Boolean`, `Character`) whose primitive value is taken right back:

* explicitly, `Integer.valueOf(text).intValue()`;
* implicitly, by assigning or returning it where the matching primitive is expected
  (`int n = Integer.valueOf(text);`, `return Long.valueOf(s);` in a method returning `long`).

It also detects other temporary objects that exist only for one call a static method or a
literal makes without them:

* a primitive boxed only to call `toString()`, `hashCode()` or `compareTo(..)` on the wrapper
  (`Integer.valueOf(n).toString()`), which `Integer.toString(n)`, `Integer.hashCode(n)` and
  `Integer.compare(a, b)` do directly;
* `String.valueOf(x)` as an operand of a string concatenation, which converts `x` itself;
* `new Foo().getClass()`, which `Foo.class` names without creating an object.

**Why it matters.** Parsing or converting through the wrapper allocates (outside the small
value cache) and reads as if an object were needed. `Integer.parseInt(..)` and its siblings
say what is meant and return the primitive directly; for a primitive argument the round trip
is simply the argument itself. The other temporary objects are the same waste: an allocation
and a detour the reader has to see through.

**How to fix.** Call the parse method (`Integer.parseInt(s)`, `Double.parseDouble(s)`,
`Boolean.parseBoolean(s)`), or use the primitive value as is; call the static helper; drop
the `String.valueOf`; use the class literal.

```java
// Noncompliant
int port = Integer.valueOf(text).intValue();
long id = Long.valueOf(raw);
String label = Integer.valueOf(count).toString();
String line = "total: " + String.valueOf(total);

// Compliant
int port = Integer.parseInt(text);
long id = Long.parseLong(raw);
String label = Integer.toString(count);
String line = "total: " + total;
```

Not reported: unboxing to a different primitive (`Integer.valueOf(s).longValue()`, which
widens), `new Integer(..)` (see `java/boxed-primitive-constructor`), wrappers whose class is
not the JDK's, wrappers built from a string before `toString()` (the round trip normalizes
the text), `String.valueOf` of an array (`char[]` prints its characters, concatenation would
print the array), next to a non-string operand (removing it would turn the `+` into an
addition) or around an expression with operators (removing it needs parentheses), `getClass()` on objects built with constructor arguments (the constructor may do
work) or anonymous classes, and boxed functional-interface types in signatures (an API
choice)."#,
    ..RuleMeta::new(
        "java/boxing-roundtrip",
        "java",
        "Throwaway boxing or temporary object",
    )
};

pub struct BoxingRoundtrip;

/// The wrapper's simple name, its primitive, unboxing method and parse method.
const WRAPPERS: &[(&str, &str, &str, Option<&str>)] = &[
    ("Integer", "int", "intValue", Some("parseInt")),
    ("Long", "long", "longValue", Some("parseLong")),
    ("Double", "double", "doubleValue", Some("parseDouble")),
    ("Float", "float", "floatValue", Some("parseFloat")),
    ("Short", "short", "shortValue", Some("parseShort")),
    ("Byte", "byte", "byteValue", Some("parseByte")),
    ("Boolean", "boolean", "booleanValue", Some("parseBoolean")),
    ("Character", "char", "charValue", None),
];

impl<'t> JavaRule<'t> for BoxingRoundtrip {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["valueOf", "getClass"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        match ctx.call_name(node) {
            Some("valueOf") => {}
            Some("getClass") => {
                check_get_class(ctx, node);
                return;
            }
            _ => return,
        }
        let Some(recv) = node.field(Field::Object) else {
            return;
        };
        let simple = util::last_segment(ctx.text(recv));
        if simple == "String" {
            check_string_value_of(ctx, node, recv);
            return;
        }
        let Some(&(wrapper, prim, unbox, parse)) = WRAPPERS.iter().find(|w| w.0 == simple) else {
            return;
        };
        let n = util::arg_count(node);
        if n == 0
            || n > 2
            || (n == 2
                && wrapper != "Integer"
                && wrapper != "Long"
                && wrapper != "Short"
                && wrapper != "Byte")
        {
            return;
        }
        let Some(parent) = ctx.parent(node) else {
            return;
        };
        // A wrapper built only to call `toString()`, `hashCode()` or `compareTo(..)`, which
        // the wrapper class offers as static methods taking the primitive.
        if n == 1
            && kind(parent) == "method_invocation"
            && parent.field(Field::Object).map(|o| o.id()) == Some(node.id())
        {
            if let Some(helper) = ctx
                .call_name(parent)
                .and_then(|m| static_helper(m, util::arg_count(parent)))
            {
                check_wrapper_helper(ctx, node, parent, recv, wrapper, prim, helper);
                return;
            }
        }
        // Where the primitive is taken back.
        let explicit = kind(parent) == "method_invocation"
            && parent.field(Field::Object).map(|o| o.id()) == Some(node.id())
            && ctx.call_name(parent) == Some(unbox)
            && util::arg_count(parent) == 0;
        if !explicit && !expects_primitive(ctx, node, parent, prim) {
            return;
        }
        if !util::names_type(ctx, recv, &format!("java.lang.{wrapper}")) {
            return;
        }
        let Some(arg) = util::arg(node, 0) else {
            return;
        };
        let arg_type = ctx.type_of(arg);
        let is_text = arg_type.as_ref().is_some_and(|t| t.is_string());
        let msg = match (is_text, parse) {
            (true, Some(parse)) => format!(
                "Call `{wrapper}.{parse}(..)` instead of boxing with `{wrapper}.valueOf(..)` and unboxing the result"
            ),
            _ if arg_type.as_ref().is_some_and(|t| t.to_string() == prim) && n == 1 => format!(
                "Use the value directly: boxing it with `{wrapper}.valueOf(..)` and unboxing it again gives the same `{prim}`"
            ),
            (_, Some(parse)) => format!(
                "Avoid boxing with `{wrapper}.valueOf(..)` only to unbox the result: use `{wrapper}.{parse}(..)` or the `{prim}` value itself"
            ),
            (_, None) => format!(
                "Use the value directly: boxing it with `{wrapper}.valueOf(..)` and unboxing it again gives the same `{prim}`"
            ),
        };
        ctx.emit(node, msg);
    }
}

/// The static wrapper method replacing an instance call `name` with `n` arguments on a
/// freshly boxed value.
fn static_helper(name: &str, n: usize) -> Option<&'static str> {
    match (name, n) {
        ("toString", 0) => Some("toString"),
        ("hashCode", 0) => Some("hashCode"),
        ("compareTo", 1) => Some("compare"),
        _ => None,
    }
}

/// `Integer.valueOf(i).toString()` and friends: only for an argument of the wrapper's own
/// primitive type (a string argument would be parsed first).
fn check_wrapper_helper<'t>(
    ctx: &Ctx<'t>,
    node: Node<'t>,
    parent: Node<'t>,
    recv: Node<'t>,
    wrapper: &str,
    prim: &str,
    helper: &str,
) {
    let Some(arg) = util::arg(node, 0) else {
        return;
    };
    if ctx.type_of(arg).is_none_or(|t| t.to_string() != prim) {
        return;
    }
    if helper == "compare" {
        // The other operand must be the same primitive or its wrapper.
        let Some(other) = util::arg(parent, 0) else {
            return;
        };
        let ok = ctx
            .type_of(other)
            .is_some_and(|t| t.to_string() == prim || t.is_class(&format!("java.lang.{wrapper}")));
        if !ok {
            return;
        }
    }
    if !util::names_type(ctx, recv, &format!("java.lang.{wrapper}")) {
        return;
    }
    let Some(method) = parent.field(Field::Name) else {
        return;
    };
    ctx.emit_range(
        Span::cover(node, method),
        format!(
            "Call `{wrapper}.{helper}(..)` on the `{prim}` instead of boxing it only to call \
             `{}`",
            ctx.text(method)
        ),
    );
}

/// `"id: " + String.valueOf(n)`: string concatenation converts the value already.
fn check_string_value_of<'t>(ctx: &Ctx<'t>, node: Node<'t>, recv: Node<'t>) {
    if util::arg_count(node) != 1 {
        return;
    }
    let Some(parent) = ctx.parent(node) else {
        return;
    };
    if kind(parent) != "binary_expression" || syntax::operator_str(parent) != "+" {
        return;
    }
    let (Some(left), Some(right)) = (parent.field(Field::Left), parent.field(Field::Right)) else {
        return;
    };
    let other = if left.id() == node.id() { right } else { left };
    // The other operand must already be a string, so that the `+` stays a concatenation.
    let other_inner = syntax::strip_parens(other);
    if kind(other_inner) == "method_invocation" && ctx.call_name(other_inner) == Some("valueOf") {
        return;
    }
    if !ctx.type_of(other).is_some_and(|t| t.is_string()) {
        return;
    }
    let Some(arg) = util::arg(node, 0) else {
        return;
    };
    // `"n=" + String.valueOf(i + 1)`: dropping the call without adding parentheses would
    // concatenate `i` and `1`; leave operators alone.
    if matches!(
        kind(syntax::strip_parens(arg)),
        "binary_expression" | "ternary_expression" | "assignment_expression"
    ) {
        return;
    }
    // `String.valueOf(char[])` prints the characters, concatenation would print the array.
    let Some(arg_type) = ctx.type_of(arg) else {
        return;
    };
    if arg_type.is_array() || arg_type.is_null() {
        return;
    }
    if !util::names_type(ctx, recv, "java.lang.String") {
        return;
    }
    ctx.emit(
        node,
        "Remove this `String.valueOf(..)`: string concatenation already converts the value",
    );
}

/// `new Foo().getClass()`: `Foo.class` names the class without creating an object.
fn check_get_class<'t>(ctx: &Ctx<'t>, node: Node<'t>) {
    if util::arg_count(node) != 0 {
        return;
    }
    let Some(obj) = node.field(Field::Object).map(syntax::strip_parens) else {
        return;
    };
    if kind(obj) != "object_creation_expression"
        || util::arg_count(obj) != 0
        || syntax::child_of_kind(obj, "class_body").is_some()
    {
        return;
    }
    let Some(ty) = obj.field(Field::Type) else {
        return;
    };
    let base = if kind(ty) == "generic_type" {
        syntax::first_named_child(ty).unwrap_or(ty)
    } else {
        ty
    };
    if !matches!(kind(base), "type_identifier" | "scoped_type_identifier") {
        return;
    }
    let name = ctx.text(base);
    ctx.emit(
        node,
        format!("Use `{name}.class` instead of creating an instance only to call `getClass()`"),
    );
}

/// Whether the call's value lands where the primitive `prim` is declared: the initializer of
/// a `prim` variable, or the operand of `return` in a method returning `prim`.
fn expects_primitive<'t>(ctx: &Ctx<'t>, call: Node<'t>, parent: Node<'t>, prim: &str) -> bool {
    match kind(parent) {
        "variable_declarator" => {
            if parent.field(Field::Value).map(|v| v.id()) != Some(call.id()) {
                return false;
            }
            let Some(decl) = ctx.parent(parent) else {
                return false;
            };
            matches!(
                kind(decl),
                "local_variable_declaration" | "field_declaration"
            ) && decl.field(Field::Type).is_some_and(|t| ctx.text(t) == prim)
                && parent.field(Field::Dimensions).is_none()
        }
        "return_statement" => ctx
            .enclosing_function(call)
            .filter(|f| kind(*f) == "method_declaration")
            .and_then(|m| m.field(Field::Type))
            .is_some_and(|t| ctx.text(t) == prim),
        _ => false,
    }
}
