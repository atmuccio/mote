//! `java/array-to-string` — `arr.toString()`, `String.valueOf(arr)`, `"" + arr`.

use super::util::{self, Fld, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["arrays", "strings"],
    remediation_minutes: 2,
    description: r#"Detects an array converted to a string with `toString()`, `String.valueOf(..)`,
or string concatenation (`"ids: " + ids`, `s += ids`).

**Why it matters.** Arrays don't override `toString()`: the result is the type code and
identity hash, e.g. `[I@1b6d3586`, never the contents. Log messages, error texts, and keys built
this way are useless or, worse, silently wrong.

**How to fix.** Use `Arrays.toString(arr)` (or `Arrays.deepToString(arr)` for nested arrays);
for a `char[]` that holds text, `new String(chars)` or `String.valueOf(chars)`.

```java
// Noncompliant
log.info("ids: " + ids);
String key = bytes.toString();

// Compliant
log.info("ids: " + Arrays.toString(ids));
String key = Arrays.toString(bytes);
```

`String.valueOf(char[])` is not reported: that overload returns the characters. Only operands
whose type the resolver proves to be an array are reported."#,
    ..RuleMeta::new(
        "java/array-to-string",
        "java",
        "Array converted to a string",
    )
};

/// How an array may be turned into a string.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Use {
    /// `arr.toString()`: the receiver; `other` is the call.
    ToString,
    /// `String.valueOf(arr)`: the argument; `other` is the `String` receiver.
    ValueOf,
    /// `"…" + arr`: the operand; `other` is the other operand.
    Concat,
    /// `s += arr`: the right-hand side; `other` is the target.
    AppendAssign,
}

/// Collects the names declared with an array type while walking, and checks the candidate
/// conversions in `finish`: only operands whose name was declared as an array (or calls to
/// methods returning arrays, casts to arrays) reach the resolver, which keeps the rule cheap on
/// the very common `+` operator.
#[derive(Default)]
pub struct ArrayToString<'t> {
    array_names: FxHashSet<&'t str>,
    array_methods: FxHashSet<&'t str>,
    candidates: Vec<(Node<'t>, Node<'t>, Use)>,
}

/// JDK methods that commonly return arrays.
const JDK_ARRAY_METHODS: &[&str] = &[
    "toArray",
    "split",
    "getBytes",
    "toCharArray",
    "clone",
    "values",
    "listFiles",
    "list",
    "getStackTrace",
    "digest",
];

const MSG: &str =
    "Use Arrays.toString(..) to show the elements; an array's own toString() gives its type and hash";

impl<'t> ArrayToString<'t> {
    /// Syntactic pre-filter: could this operand be an array? (Evaluated in `finish`, when all
    /// declarations of the file are known.)
    fn maybe_array(&self, ctx: &Ctx<'t>, n: Node<'t>) -> bool {
        match kind(n) {
            "identifier" => self.array_names.contains(ctx.text(n)),
            "field_access" => n
                .fld(F::Field)
                .is_some_and(|f| self.array_names.contains(ctx.text(f))),
            "array_access" => {
                // `grid[0]` of a multi-dimensional array: look at the base name.
                let mut base = n;
                let mut steps = 0;
                while kind(base) == "array_access" && steps < 16 {
                    match base.fld(F::Array) {
                        Some(b) => base = b,
                        None => return false,
                    }
                    steps += 1;
                }
                kind(base) != "array_access" && self.maybe_array(ctx, base)
            }
            "method_invocation" => {
                let name = util::method_name(ctx, n);
                JDK_ARRAY_METHODS.contains(&name) || self.array_methods.contains(name)
            }
            "cast_expression" => n.fld(F::Type).is_some_and(|t| kind(t) == "array_type"),
            "parenthesized_expression" | "ternary_expression" => true,
            _ => false,
        }
    }

    /// Records the variables or methods declared with an array type. Called for the rare
    /// `array_type`, `dimensions` (C-style `int x[]`), and `spread_parameter` nodes, so plain
    /// declarations cost nothing.
    fn collect_declaration(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if kind(node) == "spread_parameter" {
            // `T... xs` is an array; the name sits in a variable_declarator.
            if let Some(name) = named_children(node)
                .find(|c| kind(*c) == "variable_declarator")
                .and_then(|d| d.fld(F::Name))
            {
                self.array_names.insert(ctx.text(name));
            }
            return;
        }
        let Some(parent) = ctx.parent(node) else {
            return;
        };
        let field = if kind(node) == "array_type" {
            F::Type
        } else {
            F::Dimensions
        };
        if parent.fld(field).is_none_or(|c| c.id() != node.id()) {
            return;
        }
        match kind(parent) {
            "local_variable_declaration" | "field_declaration" | "constant_declaration" => {
                for d in syntax::declarators(parent) {
                    if let Some(name) = d.fld(F::Name) {
                        self.array_names.insert(ctx.text(name));
                    }
                }
            }
            "formal_parameter" | "enhanced_for_statement" | "variable_declarator" => {
                if let Some(name) = parent.fld(F::Name) {
                    self.array_names.insert(ctx.text(name));
                }
            }
            "method_declaration" => {
                if let Some(name) = parent.fld(F::Name) {
                    self.array_methods.insert(ctx.text(name));
                }
            }
            _ => {}
        }
    }
}

impl<'t> JavaRule<'t> for ArrayToString<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "method_invocation",
            "binary_expression",
            "assignment_expression",
            "array_type",
            "dimensions",
            "spread_parameter",
        ]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["toString", "valueOf"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        match kind(node) {
            "method_invocation" => {
                // Text pre-filter (no tree access): `….toString()` or `String.valueOf(…)`.
                let text = ctx.text(node);
                if !text.ends_with("toString()")
                    && !text.starts_with("String.valueOf")
                    && !text.starts_with("java.lang.String.valueOf")
                {
                    return;
                }
                let name = util::method_name(ctx, node);
                if name == "toString" {
                    if util::arg_count(node) != 0 {
                        return;
                    }
                    if let Some(recv) = util::plain_receiver(node) {
                        self.candidates.push((recv, node, Use::ToString));
                    }
                } else if name == "valueOf" {
                    let mut args = util::args(node);
                    let (Some(arg), None) = (args.next(), args.next()) else {
                        return;
                    };
                    let Some(recv) = node.fld(F::Object) else {
                        return;
                    };
                    if matches!(ctx.text(recv), "String" | "java.lang.String") {
                        self.candidates.push((arg, recv, Use::ValueOf));
                    }
                }
            }
            "binary_expression" => {
                if util::op_str(node) != "+" {
                    return;
                }
                let (Some(left), Some(right)) = (node.fld(F::Left), node.fld(F::Right)) else {
                    return;
                };
                for (arr, other) in [(left, right), (right, left)] {
                    if matches!(
                        kind(arr),
                        "identifier"
                            | "field_access"
                            | "method_invocation"
                            | "array_access"
                            | "cast_expression"
                            | "parenthesized_expression"
                            | "ternary_expression"
                    ) {
                        self.candidates.push((arr, other, Use::Concat));
                    }
                }
            }
            "assignment_expression" => {
                if util::op_str(node) != "+=" {
                    return;
                }
                let (Some(left), Some(right)) = (node.fld(F::Left), node.fld(F::Right)) else {
                    return;
                };
                if !util::is_literal(right) {
                    self.candidates.push((right, left, Use::AppendAssign));
                }
            }
            _ => self.collect_declaration(node, ctx),
        }
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        let candidates = std::mem::take(&mut self.candidates);
        // Bounded resolver work per file (each query on a deep node costs one descent from the
        // root; a generated file with thousands of array concatenations stays linear).
        let mut budget = 4096u32;
        for (arr, other, how) in candidates {
            if !self.maybe_array(ctx, arr) {
                continue;
            }
            if budget == 0 {
                break;
            }
            budget -= 1;
            let Some(t) = ctx.type_of(arr).filter(|t| t.is_array()) else {
                continue;
            };
            let report = match how {
                Use::ToString => true,
                Use::ValueOf => {
                    // `String.valueOf(char[])` returns the characters.
                    !t.element_type()
                        .is_some_and(|e| e.prim() == Some(Prim::Char))
                        && ctx.type_name_of(other).as_deref() == Some("java.lang.String")
                }
                Use::Concat | Use::AppendAssign => {
                    ctx.type_of(other).is_some_and(|o| o.is_string())
                }
            };
            if !report {
                continue;
            }
            let at = if how == Use::ToString {
                other.fld(F::Name).unwrap_or(arr)
            } else {
                arr
            };
            ctx.emit(at, MSG);
        }
    }
}
