//! `java/string-reference-equality` — a resolver-based rule.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["strings", "equality"],
    remediation_minutes: 2,
    description: r#"Detects `==` and `!=` between two expressions whose static type is `String`, and
between a string literal and any expression that is not known to have another type.

**Why it matters.** For objects, `==` compares references, not contents. Two strings with the
same characters are often distinct objects (anything built at run time, read from input, or
returned by a library), so the comparison can be `false` even when the text is equal. Code that
"works" with literals in tests then fails in production.

**How to fix.** Compare contents with `equals()` (or `Objects.equals(a, b)` when either side may
be `null`, or `equalsIgnoreCase()`).

```java
// Noncompliant
if (status == "ACTIVE") { ... }

// Compliant
if ("ACTIVE".equals(status)) { ... }
if (Objects.equals(a, b)) { ... }
```

Comparisons with `null`, with the result of `intern()`, and with a variable initialized with
`new String(..)` are not reported: such a sentinel (`static final String NOT_FOUND = new
String("?")`) is created as a distinct object precisely so that `==` can tell it apart from
every other string, and `equals` would change the behaviour."#,
    ..RuleMeta::new(
        "java/string-reference-equality",
        "java",
        "Strings compared with == or !=",
    )
};

pub struct StringReferenceEquality;

/// A numeric, character, or boolean literal (parentheses skipped).
fn is_primitive_literal(n: Node<'_>) -> bool {
    matches!(
        kind(syntax::strip_parens(n)),
        "decimal_integer_literal"
            | "hex_integer_literal"
            | "octal_integer_literal"
            | "binary_integer_literal"
            | "decimal_floating_point_literal"
            | "hex_floating_point_literal"
            | "character_literal"
            | "true"
            | "false"
    )
}

fn is_intern_call(ctx: &Ctx<'_>, n: Node<'_>) -> bool {
    let n = syntax::strip_parens(n);
    kind(n) == "method_invocation"
        && n.field(Field::Name)
            .is_some_and(|m| ctx.text(m) == "intern")
}

/// A variable (local or field, also `this.name`) initialized with `new String(..)`: an identity
/// sentinel meant to be compared with `==`.
fn is_identity_sentinel<'t>(ctx: &Ctx<'t>, n: Node<'t>) -> bool {
    let n = syntax::strip_parens(n);
    let ident = match kind(n) {
        "identifier" => n,
        "field_access" => match (n.field(Field::Object), n.field(Field::Field)) {
            (Some(o), Some(f)) if kind(o) == "this" && kind(f) == "identifier" => f,
            _ => return false,
        },
        _ => return false,
    };
    ctx.declaration_of(ident)
        .and_then(|d| d.value)
        .map(syntax::strip_parens)
        .is_some_and(|v| {
            kind(v) == "object_creation_expression"
                && v.field(Field::Type)
                    .is_some_and(|t| matches!(ctx.text(t), "String" | "java.lang.String"))
        })
}

impl<'t> JavaRule<'t> for StringReferenceEquality {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["binary_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let op = ctx.operator_str(node);
        if op != "==" && op != "!=" {
            return;
        }
        let (Some(left), Some(right)) = (node.field(Field::Left), node.field(Field::Right)) else {
            return;
        };
        if syntax::is_null_literal(left) || syntax::is_null_literal(right) {
            return;
        }
        if is_intern_call(ctx, left) || is_intern_call(ctx, right) {
            return;
        }
        let (left_literal, right_literal) = (
            syntax::is_string_literal(left),
            syntax::is_string_literal(right),
        );
        if left_literal || right_literal {
            // A string literal compared by reference is suspicious on its own: report unless the
            // other side is known to be something other than a `String` (an `Object` identity
            // check, say).
            let other = if left_literal { right } else { left };
            if ctx.type_of(other).is_some_and(|t| !t.is_string()) {
                return;
            }
        } else {
            // `x == 0`, `c == 'a'`, `b == true`: a primitive literal is never a String, and it
            // is cheaper to see than the type of the other side.
            if is_primitive_literal(right) || is_primitive_literal(left) {
                return;
            }
            let Some(lt) = ctx.type_of(left) else { return };
            if !lt.is_string() {
                return;
            }
            let Some(rt) = ctx.type_of(right) else { return };
            if !rt.is_string() {
                return;
            }
        }
        if is_identity_sentinel(ctx, left) || is_identity_sentinel(ctx, right) {
            return;
        }
        let at = ctx.operator(node).unwrap_or(node);
        let msg = if op == "==" {
            "Compare these strings with equals() instead of =="
        } else {
            "Compare these strings with !equals() instead of !="
        };
        ctx.emit(at, msg);
    }
}
