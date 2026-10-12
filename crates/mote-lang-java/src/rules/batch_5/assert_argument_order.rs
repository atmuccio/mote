//! `java/assert-argument-order` — `assertEquals(actual, "expected")`: the constant belongs first.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["tests", "junit"],
    remediation_minutes: 1,
    description: r#"Detects JUnit equality assertions (`assertEquals`, `assertNotEquals`,
`assertSame`, `assertNotSame`, `assertArrayEquals`) whose *actual* slot holds a constant — a
literal, a class literal, an enum constant, or a `static final` field — while the *expected*
slot holds a computed value.

**Why it matters.** JUnit's parameters are `(expected, actual)`. The assertion still passes or
fails the same way when they are swapped, but the failure message becomes misleading:
`expected:<[computed value]> but was:<[the constant]>` sends the reader looking for the bug in
the wrong place.

**How to fix.** Put the constant first.

```java
// Noncompliant
assertEquals(order.getStatus(), Status.SHIPPED);
assertEquals(items.size(), 3);

// Compliant
assertEquals(Status.SHIPPED, order.getStatus());
assertEquals(3, items.size());
```

Only test files are checked, and only calls that provably go to JUnit 3/4 (`org.junit.Assert`,
message first) or JUnit 5 (`org.junit.jupiter.api.Assertions`, message last) — TestNG uses the
opposite order and is left alone. Not reported: both arguments constant, boolean or `null`
literals (other assertions fit better), a first argument named `expected…`, files that
declare their own assertion helper of the same name, and values of unrelated types
(`assertNotEquals(point, "42")` with `point` declared as a class of the project): such an
assertion tests the `equals` of the object in the first slot against a value of another type,
and swapping the arguments would test the constant's `equals` instead."#,
    ..RuleMeta::new(
        "java/assert-argument-order",
        "java",
        "Expected and actual values swapped in an assertion",
    )
};

const ASSERTS: &[&str] = &[
    "assertEquals",
    "assertNotEquals",
    "assertSame",
    "assertNotSame",
    "assertArrayEquals",
];

/// Where the message parameter goes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Order {
    /// JUnit 3/4: `(message, expected, actual)`.
    MessageFirst,
    /// JUnit 5: `(expected, actual, message)`.
    MessageLast,
}

fn order_of(owner: &str) -> Option<Order> {
    match owner {
        "org.junit.Assert"
        | "junit.framework.Assert"
        | "junit.framework.TestCase"
        | "org.testng.AssertJUnit" => Some(Order::MessageFirst),
        "org.junit.jupiter.api.Assertions" => Some(Order::MessageLast),
        _ => None,
    }
}

/// Per-file state: the resolution of unqualified assertion calls, cached per enclosing class
/// and method name (a test class holds hundreds of them).
#[derive(Default)]
pub struct AssertArgumentOrder<'t> {
    unqualified: Vec<(usize, &'t str, Option<Order>)>,
}

impl<'t> AssertArgumentOrder<'t> {
    /// The assertion class an unqualified call goes to, from the static imports (or a JUnit 3
    /// `TestCase` superclass).
    fn unqualified_owner(ctx: &Ctx<'t>, call: Node<'t>, name: &str) -> Option<Order> {
        let mut found: Option<Order> = None;
        for i in ctx.imports().iter().filter(|i| i.is_static) {
            let owner = if i.is_wildcard {
                i.path
            } else if i.simple_name() == name {
                i.qualifier()
            } else {
                continue;
            };
            let Some(o) = order_of(owner) else {
                // A static import of the same name from elsewhere (TestNG, a helper): unsure.
                if !i.is_wildcard {
                    return None;
                }
                continue;
            };
            if found.is_some_and(|f| f != o) {
                return None;
            }
            found = Some(o);
        }
        if found.is_some() {
            return found;
        }
        // JUnit 3: inherited from `junit.framework.TestCase`.
        let class = ctx.enclosing_class(call)?;
        let sup = util::fld(class, F.superclass)?;
        let ty = syntax::first_named_child(sup)?;
        match util::type_fqn(ctx, ty).as_deref() {
            Some("junit.framework.TestCase") => Some(Order::MessageFirst),
            _ => None,
        }
    }

    fn is_string_like(ctx: &Ctx<'t>, n: Node<'t>) -> Option<bool> {
        if syntax::is_string_literal(n) {
            return Some(true);
        }
        ctx.type_of(n).map(|t| t.is_string())
    }
}

/// Whether the constant in the *actual* slot has a final type (`String`, a wrapper, a JDK
/// final class) that the *expected* value's declared class is neither equal to nor a supertype
/// of: the two can never be equal, so the assertion exercises the first value's `equals` on a
/// foreign type (the equals-contract test `assertNotEquals(point, "42")`), where the argument
/// order is deliberate. Numeric and boolean values are left alone (JUnit compares them by
/// value through its primitive overloads).
fn unrelated_types<'t>(ctx: &Ctx<'t>, expected: Node<'t>, actual: Node<'t>) -> bool {
    let Some(ta) = ctx.type_of(actual) else {
        return false;
    };
    if ta.is_numeric() || ta.is_boolean_like() || ta.unboxed().is_some() || ta.is_primitive() {
        return false;
    }
    let Some(fa) = ta.fqn() else {
        return false;
    };
    if !crate::resolve::is_final_class(fa) {
        return false;
    }
    let fe = match ctx.type_of(expected) {
        Some(te) => {
            if te.is_numeric() || te.is_boolean_like() || te.is_primitive() {
                return false;
            }
            match te.fqn() {
                Some(f) => f.to_string(),
                None => return false,
            }
        }
        None => {
            // A variable declared with a class the resolver does not model (a project class).
            if kind(expected) != "identifier" {
                return false;
            }
            let Some(ty) = ctx
                .declaration_of(expected)
                .filter(|d| !d.is_var)
                .and_then(|d| d.type_node)
            else {
                return false;
            };
            let base = util::type_base(ty);
            if !matches!(kind(base), "type_identifier" | "scoped_type_identifier") {
                return false;
            }
            let name = ctx.text(base);
            match ctx.resolve_type_name(name, expected) {
                Some(f) => f,
                // Not a class of this file, an import or the JDK table: a class of the same
                // package, which a final JDK class cannot extend — unless it is a type variable.
                None if !name.contains('.') && !is_type_variable(ctx, expected, name) => {
                    name.to_string()
                }
                None => return false,
            }
        }
    };
    fe != fa && ctx.subtype(&ta, &fe) == Some(false)
}

/// Whether `name` is a type parameter of a method or class enclosing `at`.
fn is_type_variable<'t>(ctx: &Ctx<'t>, at: Node<'t>, name: &str) -> bool {
    ctx.ancestors(at).any(|a| {
        (syntax::is_method_like(a) || syntax::is_type_declaration(a))
            && a.field(Field::TypeParameters).is_some_and(|tp| {
                named_children(tp).any(|p| {
                    named_children(p).any(|c| kind(c) == "type_identifier" && ctx.text(c) == name)
                })
            })
    })
}

impl<'t> JavaRule<'t> for AssertArgumentOrder<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        ASSERTS
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !ctx.is_test() {
            return;
        }
        let Some(name) = util::call_named(ctx, node, ASSERTS) else {
            return;
        };
        let n = util::arg_count(node);
        if !(2..=4).contains(&n) {
            return;
        }
        let order = match util::fld(node, F.object) {
            Some(obj) => match ctx.type_name_of(obj) {
                Some(owner) => order_of(&owner),
                None => None,
            },
            None => {
                let class = ctx.enclosing_class(node);
                let key = class.map_or(0, |c| c.id());
                match self
                    .unqualified
                    .iter()
                    .find(|(k, n, _)| *k == key && *n == name)
                {
                    Some(&(_, _, o)) => o,
                    None => {
                        let own = class
                            .and_then(syntax::type_body)
                            .is_some_and(|body| util::declares_method(ctx, body, name));
                        let o = if own {
                            None
                        } else {
                            Self::unqualified_owner(ctx, node, name)
                        };
                        self.unqualified.push((key, name, o));
                        o
                    }
                }
            }
        };
        let Some(order) = order else {
            return;
        };
        let first = match (order, n) {
            (Order::MessageLast, _) | (Order::MessageFirst, 2) => 0,
            (Order::MessageFirst, 4) => 1,
            (Order::MessageFirst, _) => {
                // (message, expected, actual) or (expected, actual, delta).
                let Some(a0) = util::arg(node, 0) else {
                    return;
                };
                match Self::is_string_like(ctx, a0) {
                    Some(true) => 1,
                    Some(false)
                        if util::arg(node, 2)
                            .and_then(|d| ctx.type_of(d))
                            .is_some_and(|t| t.is_numeric()) =>
                    {
                        0
                    }
                    _ => return,
                }
            }
        };
        let (Some(expected), Some(actual)) = (util::arg(node, first), util::arg(node, first + 1))
        else {
            return;
        };
        let actual_inner = syntax::strip_parens(actual);
        if matches!(kind(actual_inner), "true" | "false" | "null_literal") {
            return;
        }
        if !util::is_constant_like(ctx, actual) || util::is_constant_like(ctx, expected) {
            return;
        }
        let e = syntax::strip_parens(expected);
        if kind(e) == "identifier" && ctx.text(e).to_ascii_lowercase().starts_with("expected") {
            return;
        }
        if unrelated_types(ctx, e, actual) {
            return;
        }
        ctx.emit(
            actual,
            format!(
                "Swap the arguments of `{name}`: the expected value `{}` should come first",
                util::shorten(ctx.text(actual), 40)
            ),
        );
    }
}
