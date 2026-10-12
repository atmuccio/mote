//! `java/boxed-primitive-constructor` — `new Integer(..)` and friends.

use super::util::{Fld, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["performance", "deprecated"],
    remediation_minutes: 2,
    description: r#"Detects `new Integer(..)`, `new Long(..)`, `new Short(..)`, `new Byte(..)`,
`new Double(..)`, `new Float(..)`, `new Boolean(..)`, and `new Character(..)`; also other
constructors that only rebuild a value the JDK already offers: `new String("text")` and
`new String()` (the literal is already a `String`), and `BigInteger` / `BigDecimal` built from
a value the class has a constant for (`new BigDecimal(1)`, `new BigInteger("10")`: use
`ONE`, `TEN`, `ZERO`, and `TWO` where it exists — `BigInteger.TWO` from Java 9,
`BigDecimal.TWO` from Java 19), which gives the same value without parsing or allocating.

**Why it matters.** These constructors are deprecated for removal. Each call allocates a new
object, bypassing the caches that `valueOf` uses, and invites identity bugs: two boxes built
this way are never `==` even when their values are equal.

**How to fix.** Use the static factory (`Integer.valueOf(..)`, `Boolean.valueOf(..)`,
`Character.valueOf(..)`), the parser (`Integer.parseInt(..)`) when a primitive is enough, or
plain autoboxing.

```java
// Noncompliant
Integer count = new Integer(0);
Boolean flag = new Boolean(text);

// Compliant
Integer count = Integer.valueOf(0);
boolean flag = Boolean.parseBoolean(text);
```

Classes of your own that happen to be called `Integer`, `Long`, … are not reported. Neither
are `new String(..)` in test code (tests build distinct instances on purpose) or one assigned
to a variable that the file compares with `==` or `!=` (an identity sentinel), nor
`BigDecimal` values with a fraction or exponent (`new BigDecimal("1.0")` has another scale
than `ONE`), nor other whole numbers (`new BigDecimal("19")` reads well and `valueOf` would
gain little)."#,
    ..RuleMeta::new(
        "java/boxed-primitive-constructor",
        "java",
        "Deprecated wrapper-class constructor",
    )
};

pub struct BoxedPrimitiveConstructor;

const WRAPPERS: &[&str] = &[
    "Integer",
    "Long",
    "Short",
    "Byte",
    "Double",
    "Float",
    "Boolean",
    "Character",
];

/// The value of a whole-number literal argument: `7`, `7L`, `"7"` (no sign, fraction or
/// exponent); `None` otherwise.
fn whole_number(ctx: &Ctx<'_>, arg: Node<'_>, allow_int: bool) -> Option<i64> {
    let arg = syntax::strip_parens(arg);
    let text = match kind(arg) {
        "decimal_integer_literal" if allow_int => ctx.text(arg).trim_end_matches(['l', 'L']),
        "string_literal" => {
            let t = ctx.text(arg);
            t.strip_prefix('"')?.strip_suffix('"')?
        }
        _ => return None,
    };
    let digits = text.replace('_', "");
    if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    // `007` in a string still parses as 7, but keep the advice simple.
    if digits.len() > 1 && digits.starts_with('0') {
        return None;
    }
    digits.parse::<i64>().ok()
}

/// Whether `name` is compared with `==` or `!=` somewhere in the file (an identity sentinel).
fn compared_by_identity(src: &str, name: &str) -> bool {
    let b = src.as_bytes();
    let is_word = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
    src.match_indices(name).any(|(i, _)| {
        let end = i + name.len();
        if (i > 0 && is_word(b[i - 1])) || b.get(end).is_some_and(|c| is_word(*c)) {
            return false;
        }
        let before = src[..i].trim_end();
        let after = src[end..].trim_start();
        before.ends_with("==")
            || before.ends_with("!=")
            || after.starts_with("==")
            || after.starts_with("!=")
    })
}

impl BoxedPrimitiveConstructor {
    /// `new String("text")` / `new String()`.
    fn check_string<'t>(node: Node<'t>, ty: Node<'t>, ctx: &Ctx<'t>) {
        if ctx.is_test() {
            return;
        }
        let args = node.fld(F::Arguments);
        let mut it = args.into_iter().flat_map(named_children);
        let replacement = match (it.next(), it.next()) {
            (None, _) => "\"\"".to_string(),
            (Some(a), None) if kind(a) == "string_literal" => {
                let t = ctx.text(a);
                if t.len() > 40 {
                    "the literal itself".to_string()
                } else {
                    t.to_string()
                }
            }
            _ => return,
        };
        if ctx
            .resolve_type(ty)
            .and_then(|t| t.fqn().map(str::to_string))
            .as_deref()
            != Some("java.lang.String")
        {
            return;
        }
        // An identity sentinel: `static final String NONE = new String("?")` compared with `==`.
        if let Some(decl) = ctx
            .parent(node)
            .filter(|p| kind(*p) == "variable_declarator")
        {
            if let Some(name) = decl.fld(F::Name) {
                if compared_by_identity(ctx.source(), ctx.text(name)) {
                    return;
                }
            }
        }
        ctx.emit_range(
            Span::new(node.start_byte(), ty.end_byte()),
            format!("Use {replacement} instead of copying it with `new String(..)`"),
        );
    }

    /// `new BigDecimal(1)`, `new BigInteger("10")`: a constant or `valueOf`.
    fn check_big<'t>(node: Node<'t>, ty: Node<'t>, ctx: &Ctx<'t>, simple: &str) {
        let args = node.fld(F::Arguments);
        let mut it = args.into_iter().flat_map(named_children);
        let (Some(arg), None) = (it.next(), it.next()) else {
            return;
        };
        let big_decimal = simple == "BigDecimal";
        let Some(v) = whole_number(ctx, arg, big_decimal) else {
            return;
        };
        let Some(fqn) = ctx
            .resolve_type(ty)
            .and_then(|t| t.fqn().map(str::to_string))
        else {
            return;
        };
        if fqn != format!("java.math.{simple}") {
            return;
        }
        // `TWO` arrived in Java 9 (BigInteger) and Java 19 (BigDecimal).
        let two_since = if big_decimal { 19 } else { 9 };
        let constant = match v {
            0 => "ZERO",
            1 => "ONE",
            2 if ctx.java_release().is_some_and(|r| r >= two_since) => "TWO",
            10 => "TEN",
            _ => return,
        };
        ctx.emit_range(
            Span::new(node.start_byte(), ty.end_byte()),
            format!("Use {simple}.{constant} instead of constructing the value"),
        );
    }
}

impl<'t> JavaRule<'t> for BoxedPrimitiveConstructor {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["object_creation_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(ty) = node.fld(F::Type) else {
            return;
        };
        let text = ctx.text(ty);
        let simple = text.rsplit('.').next().unwrap_or(text).trim();
        if !matches!(simple, "String" | "BigDecimal" | "BigInteger") && !WRAPPERS.contains(&simple)
        {
            return;
        }
        if syntax::child_of_kind(node, "class_body").is_some() {
            return;
        }
        match simple {
            "String" => return Self::check_string(node, ty, ctx),
            "BigDecimal" | "BigInteger" => return Self::check_big(node, ty, ctx, simple),
            _ => {}
        }
        let Some(name) = WRAPPERS.iter().find(|w| **w == simple) else {
            return;
        };
        let Some(t) = ctx.resolve_type(ty) else {
            return;
        };
        if t.fqn()
            .is_none_or(|f| f.strip_prefix("java.lang.") != Some(name))
        {
            return;
        }
        let factory = if *name == "Boolean" {
            "Boolean.valueOf(..) (or Boolean.parseBoolean(..))".to_string()
        } else {
            format!("{name}.valueOf(..)")
        };
        ctx.emit_range(
            Span::new(node.start_byte(), ty.end_byte()),
            format!("Use {factory} instead of the deprecated `new {name}(..)`"),
        );
    }
}
