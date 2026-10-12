//! `java/iterator-contract` — an `Iterator.next()` that never throws `NoSuchElementException`,
//! and an `iterator()` that returns the iterator itself.

use super::util::{visit, Walk};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["iterators", "api-misuse"],
    remediation_minutes: 10,
    description: r#"Detects two breaches of the iterator contracts in classes that implement
`java.util.Iterator`:

- a `next()` that cannot throw `NoSuchElementException`: it has no `throw` and does not
  delegate to anything that would (`return items[index++];`, `if (!hasNext()) return null;`);
- an `iterator()` method that returns `this`.

**Why it matters.** Callers and library code rely on `next()` throwing
`NoSuchElementException` once the iteration is exhausted. An implementation that returns
`null` instead lets a consumer loop on silently with a `null` element, and one that indexes
past the end throws an unrelated exception that callers don't expect. An `Iterable` whose
`iterator()` returns itself can be traversed only once: a second or nested `for` loop over it
sees no elements, or steals elements from the first.

**How to fix.** Start `next()` with `if (!hasNext()) throw new NoSuchElementException();`.
Return a new iterator object from `iterator()`.

```java
// Noncompliant
public T next() {
    return items[position++];
}

// Compliant
public T next() {
    if (position >= items.length) {
        throw new NoSuchElementException();
    }
    return items[position++];
}
```

`next()` methods that call another `next…()` (`nextInt()`, `nextToken()`), a queue method that throws when empty
(`remove()`, `pop()`, …) or a helper method of the same class are not reported: the exception
may come from there. That includes such a call in a field initializer of an anonymous object
that `next()` creates (`return new Entry<>() { final E e = it.next(); … }`), which runs during
`next()`. Infinite iterators, whose `hasNext()` is `return true;`, are not reported either:
they are never exhausted, so `next()` has no reason to throw."#,
    ..RuleMeta::new(
        "java/iterator-contract",
        "java",
        "Iterator implementation breaks the iterator contract",
    )
};

pub struct IteratorContract;

/// Calls that throw `NoSuchElementException` themselves (or delegate the iteration).
const THROWING_CALLS: &[&str] = &[
    "next",
    "nextElement",
    "previous",
    "remove",
    "removeFirst",
    "removeLast",
    "pop",
    "element",
    "getFirst",
    "getLast",
    "first",
    "last",
    "firstKey",
    "lastKey",
    "orElseThrow",
];

impl<'t> JavaRule<'t> for IteratorContract {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_declaration"]
    }

    fn method_names(&self) -> &'static [&'static str] {
        &["next", "iterator"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) = node.field(Field::Name) else {
            return;
        };
        let method = ctx.text(name);
        if method != "next" && method != "iterator" {
            return;
        }
        if syntax::parameters(node).next().is_some() {
            return;
        }
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        let Some(class) = ctx.enclosing_class(node) else {
            return;
        };
        if !implements_iterator(class, ctx) {
            return;
        }
        if method == "iterator" {
            let returns_this = named_children(body).any(|s| {
                kind(s) == "return_statement"
                    && syntax::first_named_child(s)
                        .is_some_and(|e| kind(syntax::strip_parens(e)) == "this")
            });
            if returns_this {
                ctx.emit(
                    name,
                    "Return a new iterator; returning this lets the object be traversed only once",
                );
            }
            return;
        }
        if !may_throw(body, ctx, 0) && !is_infinite(class, ctx) {
            ctx.emit(
                name,
                "Throw NoSuchElementException when the iteration is exhausted; this next() never does",
            );
        }
    }
}

/// Whether the class (or anonymous class) directly implements `java.util.Iterator` or
/// `ListIterator`.
fn implements_iterator<'t>(class: Node<'t>, ctx: &Ctx<'t>) -> bool {
    let is_iter = |t: Node<'t>| {
        let base = if kind(t) == "generic_type" {
            syntax::first_named_child(t).unwrap_or(t)
        } else {
            t
        };
        let text = ctx.text(base);
        if !text.ends_with("Iterator") {
            return false;
        }
        ctx.resolve_type(t)
            .and_then(|r| {
                r.fqn()
                    .map(|f| f == "java.util.Iterator" || f == "java.util.ListIterator")
            })
            .unwrap_or(false)
    };
    match kind(class) {
        "class_declaration" | "enum_declaration" | "record_declaration" => class
            .field(Field::Interfaces)
            .and_then(syntax::first_named_child)
            .is_some_and(|list| named_children(list).any(is_iter)),
        "object_creation_expression" => class.field(Field::Type).is_some_and(is_iter),
        _ => false,
    }
}

/// Whether code under `root` may throw `NoSuchElementException`: a `throw`, a call that throws
/// it itself or delegates the iteration, or a helper of the same class. The fields and instance
/// initializers of an object created here (`return new Entry() { final E e = it.next(); … }`)
/// run right away, so they count; methods of nested classes and lambdas do not.
fn may_throw<'t>(root: Node<'t>, ctx: &Ctx<'t>, depth: u32) -> bool {
    visit(root, |n| match kind(n) {
        "throw_statement" => Walk::Stop,
        "lambda_expression" | "class_body" => Walk::Skip,
        "object_creation_expression" => {
            let eager = depth < 4
                && syntax::child_of_kind(n, "class_body").is_some_and(|cb| {
                    ctx.members(cb).any(|m| {
                        matches!(kind(m), "field_declaration" | "block")
                            && !syntax::has_modifier(m, "static")
                            && may_throw(m, ctx, depth + 1)
                    })
                });
            if eager {
                Walk::Stop
            } else {
                Walk::Continue
            }
        }
        "method_invocation" => {
            let call = ctx.call_name(n).unwrap_or("");
            let own = n
                .field(Field::Object)
                .is_none_or(|o| matches!(kind(o), "this" | "super"));
            if THROWING_CALLS.contains(&call)
                || call.starts_with("next")
                || (own && call != "hasNext")
            {
                Walk::Stop
            } else {
                Walk::Continue
            }
        }
        _ => Walk::Continue,
    })
}

/// Whether the iterator never ends: its `hasNext()` is `return true;`. Such an iterator is
/// never exhausted, so its `next()` has no reason to throw.
fn is_infinite<'t>(class: Node<'t>, ctx: &Ctx<'t>) -> bool {
    let Some(body) = syntax::type_body(class) else {
        return false;
    };
    ctx.members(body).any(|m| {
        kind(m) == "method_declaration"
            && syntax::name_of(m, ctx.source()) == "hasNext"
            && syntax::parameters(m).next().is_none()
            && m.field(Field::Body).is_some_and(|b| {
                let mut stmts = named_children(b).filter(|s| !syntax::is_comment(*s));
                let only = stmts.next();
                stmts.next().is_none()
                    && only.is_some_and(|s| {
                        kind(s) == "return_statement"
                            && syntax::first_named_child(s)
                                .is_some_and(|e| kind(syntax::strip_parens(e)) == "true")
                    })
            })
    })
}
