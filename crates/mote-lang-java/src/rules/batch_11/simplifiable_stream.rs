//! `java/simplifiable-stream` — stream pipelines that a shorter built-in operation expresses.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["streams", "idiom"],
    remediation_minutes: 2,
    description: r#"Detects stream pipelines written the long way when the stream API (or the
collection itself) has an operation that says it directly:

* `filter(p).findFirst().isPresent()` or `filter(p).findAny().isPresent()` is `anyMatch(p)`;
  with `isEmpty()` it is `noneMatch(p)`;
* `filter(p).count() > 0` (or `!= 0`, `>= 1`) is `anyMatch(p)`; `filter(p).count() == 0`
  is `noneMatch(p)`;
* `collect(Collectors.counting())` is `count()`;
* `collection.stream().forEach(action)` is `collection.forEach(action)`;
* `forEach(list::add)` (or `forEach(x -> list.add(x))`) into a collection created empty by the
  statement just before is `collect(Collectors.toList())` (`toSet()` for a set).

**Why it matters.** The dedicated operation names the intent — "is there a match?", "how
many?", "gather them" — instead of making the reader reconstruct it from a sequence of steps.
It is also cheaper: `anyMatch` and `noneMatch` stop at the first element that decides the
answer, while `count()` visits every element; `findFirst` must also respect the encounter
order, which parallel streams pay for.

**How to fix.** Use the shorter form.

```java
// Noncompliant
boolean late = orders.stream().filter(Order::isLate).findFirst().isPresent();
if (users.stream().filter(User::isAdmin).count() > 0) { … }
List<String> names = new ArrayList<>();
users.stream().map(User::name).forEach(names::add);

// Compliant
boolean late = orders.stream().anyMatch(Order::isLate);
if (users.stream().anyMatch(User::isAdmin)) { … }
List<String> names = users.stream().map(User::name).collect(Collectors.toList());
```

Only pipelines the resolver proves to be `java.util.stream` streams (or that start with a
`stream()` call) are considered. Not reported: `parallelStream().forEach(…)` (deliberately
parallel), `count()` compared with anything other than 0 or 1, collections that already hold
elements, are used between their creation and the `forEach`, or are declared with a concrete
type (`ArrayList<String> names`, which a collector's `List` cannot be assigned to), and `collect(toList())` that
could be `toList()`: the latter returns an unmodifiable list, which is only equivalent when
nothing modifies the result."#,
    ..RuleMeta::new(
        "java/simplifiable-stream",
        "java",
        "Stream pipeline with a shorter built-in form",
    )
};

pub struct SimplifiableStream;

/// Stream operations returning a stream of the same kind of pipeline.
const INTERMEDIATE: &[&str] = &[
    "filter",
    "map",
    "mapToObj",
    "mapToInt",
    "mapToLong",
    "mapToDouble",
    "flatMap",
    "distinct",
    "sorted",
    "peek",
    "limit",
    "skip",
    "boxed",
    "takeWhile",
    "dropWhile",
    "sequential",
    "unordered",
];

/// Whether `e` is provably a `java.util.stream` stream: its resolved type, or a pipeline that
/// starts with a `stream()` / `parallelStream()` call.
fn is_stream<'t>(ctx: &Ctx<'t>, e: Node<'t>, depth: u32) -> bool {
    let e = syntax::strip_parens(e);
    if depth > 32 {
        return false;
    }
    let proven = |e: Node<'t>| {
        ctx.type_of(e)
            .is_some_and(|t| ctx.is_subtype_of(&t, "java.util.stream.BaseStream"))
    };
    if kind(e) != "method_invocation" {
        return matches!(kind(e), "identifier" | "field_access") && proven(e);
    }
    let Some(name) = ctx.call_name(e) else {
        return false;
    };
    if matches!(name, "stream" | "parallelStream") && syntax::argument_count(e) == 0 {
        return true;
    }
    if INTERMEDIATE.contains(&name) {
        if let Some(obj) = e.field(Field::Object) {
            if is_stream(ctx, obj, depth + 1) {
                return true;
            }
        }
    }
    proven(e)
}

/// For `s.filter(p)` on a stream: the `filter` call.
fn stream_filter<'t>(ctx: &Ctx<'t>, e: Node<'t>) -> Option<Node<'t>> {
    let e = syntax::strip_parens(e);
    (kind(e) == "method_invocation"
        && ctx.call_name(e) == Some("filter")
        && syntax::argument_count(e) == 1
        && e.field(Field::Object).is_some_and(|o| is_stream(ctx, o, 0)))
    .then_some(e)
}

impl SimplifiableStream {
    /// `filter(p).findFirst().isPresent()` / `.isEmpty()`.
    fn find_present<'t>(ctx: &Ctx<'t>, node: Node<'t>, name: &str) {
        let Some(find) = node.field(Field::Object).map(syntax::strip_parens) else {
            return;
        };
        if kind(find) != "method_invocation"
            || !matches!(ctx.call_name(find), Some("findFirst" | "findAny"))
            || syntax::argument_count(find) != 0
        {
            return;
        }
        let Some(filter) = find
            .field(Field::Object)
            .and_then(|o| stream_filter(ctx, o))
        else {
            return;
        };
        let replacement = if name == "isPresent" {
            "anyMatch"
        } else {
            "noneMatch"
        };
        let Some(start) = filter.field(Field::Name) else {
            return;
        };
        ctx.emit_range(
            Span::new(start.start_byte(), node.end_byte()),
            format!("Use `{replacement}(..)` instead of filtering, finding and testing the result"),
        );
    }

    /// `filter(p).count() > 0` and its variants.
    fn count_compared<'t>(ctx: &Ctx<'t>, node: Node<'t>) {
        let Some(filter) = node
            .field(Field::Object)
            .and_then(|o| stream_filter(ctx, o))
        else {
            return;
        };
        let Some(parent) = ctx.parent(node) else {
            return;
        };
        if kind(parent) != "binary_expression" {
            return;
        }
        let (Some(left), Some(right)) = (parent.field(Field::Left), parent.field(Field::Right))
        else {
            return;
        };
        let src = ctx.source();
        let count_left = left.id() == node.id();
        let other = if count_left { right } else { left };
        let Some(value) = syntax::int_literal_value(syntax::strip_parens(other), src) else {
            return;
        };
        // Normalize to `count OP value`.
        let op = match (syntax::operator_str(parent), count_left) {
            (op, true) => op,
            ("<", false) => ">",
            (">", false) => "<",
            ("<=", false) => ">=",
            (">=", false) => "<=",
            (op, false) => op,
        };
        let replacement = match (op, value) {
            (">" | "!=", 0) | (">=", 1) => "anyMatch",
            ("==" | "<=", 0) | ("<", 1) => "noneMatch",
            _ => return,
        };
        let Some(start) = filter.field(Field::Name) else {
            return;
        };
        ctx.emit_range(
            Span::new(
                start.start_byte().min(parent.start_byte()),
                parent.end_byte(),
            ),
            format!("Use `{replacement}(..)` instead of counting the matching elements"),
        );
    }

    /// `collect(Collectors.counting())`.
    fn collect_counting<'t>(ctx: &Ctx<'t>, node: Node<'t>) {
        if syntax::argument_count(node) != 1 {
            return;
        }
        let Some(arg) = syntax::arguments(node).next().map(syntax::strip_parens) else {
            return;
        };
        if kind(arg) != "method_invocation"
            || ctx.call_name(arg) != Some("counting")
            || syntax::argument_count(arg) != 0
        {
            return;
        }
        let collectors = match arg.field(Field::Object) {
            Some(o) => util::names_type(ctx, o, "java.util.stream.Collectors"),
            None => ctx.imports().iter().any(|i| {
                i.is_static
                    && (i.path == "java.util.stream.Collectors.counting"
                        || (i.is_wildcard && i.path == "java.util.stream.Collectors"))
            }),
        };
        if !collectors
            || !node
                .field(Field::Object)
                .is_some_and(|o| is_stream(ctx, o, 0))
        {
            return;
        }
        let Some(name) = node.field(Field::Name) else {
            return;
        };
        ctx.emit_range(
            Span::new(name.start_byte(), node.end_byte()),
            "Use `count()` instead of collecting with `Collectors.counting()`",
        );
    }

    /// `collection.stream().forEach(f)` and `forEach(list::add)` into a fresh collection.
    fn for_each<'t>(ctx: &Ctx<'t>, node: Node<'t>) {
        if syntax::argument_count(node) != 1 {
            return;
        }
        let Some(recv) = node.field(Field::Object).map(syntax::strip_parens) else {
            return;
        };
        let Some(action) = syntax::arguments(node).next() else {
            return;
        };
        // `forEach(list::add)` / `forEach(x -> list.add(x))` right after `list = new …()`.
        if let Some(target) = added_to(ctx, action) {
            if let Some(collection) = fresh_collection(ctx, node, target) {
                if is_stream(ctx, recv, 0) {
                    let collector = if collection == "set" {
                        "toSet"
                    } else {
                        "toList"
                    };
                    ctx.emit(
                        node,
                        format!(
                            "Collect the stream with `Collectors.{collector}()` instead of \
                             adding each element to `{target}`"
                        ),
                    );
                }
                return;
            }
        }
        // `collection.stream().forEach(f)`.
        if kind(recv) != "method_invocation"
            || ctx.call_name(recv) != Some("stream")
            || syntax::argument_count(recv) != 0
        {
            return;
        }
        let Some(coll) = recv.field(Field::Object) else {
            return;
        };
        let Some(t) = ctx.type_of(coll) else {
            return;
        };
        if !ctx.is_subtype_of(&t, "java.util.Collection") {
            return;
        }
        let Some(stream_name) = recv.field(Field::Name) else {
            return;
        };
        ctx.emit_range(
            Span::new(stream_name.start_byte(), node.end_byte()),
            "Call `forEach` on the collection itself instead of on a stream of it",
        );
    }
}

/// For `list::add` or `x -> list.add(x)`: the name `list`.
fn added_to<'t>(ctx: &Ctx<'t>, action: Node<'t>) -> Option<&'t str> {
    let action = syntax::strip_parens(action);
    match kind(action) {
        "method_reference" => {
            let text = ctx.text(action);
            let (recv, method) = text.split_once("::")?;
            let recv = recv.trim();
            (method.trim() == "add" && is_simple_name(recv)).then_some(recv)
        }
        "lambda_expression" => {
            let params = action.field(Field::Parameters)?;
            let param = match kind(params) {
                "identifier" => ctx.text(params),
                "inferred_parameters" => {
                    let mut it = named_children(params);
                    let p = it.next()?;
                    if it.next().is_some() {
                        return None;
                    }
                    ctx.text(p)
                }
                _ => return None,
            };
            let body = syntax::strip_parens(action.field(Field::Body)?);
            if kind(body) != "method_invocation" || ctx.call_name(body) != Some("add") {
                return None;
            }
            let recv = body.field(Field::Object)?;
            let mut args = syntax::arguments(body);
            let arg = args.next()?;
            if args.next().is_some()
                || kind(recv) != "identifier"
                || kind(arg) != "identifier"
                || ctx.text(arg) != param
            {
                return None;
            }
            Some(ctx.text(recv))
        }
        _ => None,
    }
}

fn is_simple_name(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
        && !s.starts_with(|c: char| c.is_ascii_digit() || c.is_uppercase())
}

/// When the statement holding `call` directly follows `Type target = new ArrayList<>();` (or
/// another empty JDK list or set): `"list"` or `"set"`.
fn fresh_collection<'t>(ctx: &Ctx<'t>, call: Node<'t>, target: &str) -> Option<&'static str> {
    let stmt = ctx.parent(call)?;
    if kind(stmt) != "expression_statement" {
        return None;
    }
    let mut prev = ctx.prev_named_sibling(stmt)?;
    while syntax::is_comment(prev) {
        prev = ctx.prev_named_sibling(prev)?;
    }
    if kind(prev) != "local_variable_declaration" {
        return None;
    }
    let mut decls = syntax::declarators(prev);
    let decl = decls.next()?;
    if decls.next().is_some() || syntax::name_of(decl, ctx.source()) != target {
        return None;
    }
    // Declared as the interface, so that the collector's result can be assigned to it.
    let declared = util::type_fqn(ctx, prev.field(Field::Type)?)?;
    if !matches!(
        declared.as_str(),
        "java.util.List" | "java.util.Set" | "java.util.Collection"
    ) {
        return None;
    }
    let value = syntax::strip_parens(decl.field(Field::Value)?);
    if kind(value) != "object_creation_expression"
        || syntax::argument_count(value) != 0
        || syntax::child_of_kind(value, "class_body").is_some()
    {
        return None;
    }
    let fqn = util::type_fqn(ctx, value.field(Field::Type)?)?;
    match fqn.as_str() {
        "java.util.ArrayList" | "java.util.LinkedList" => Some("list"),
        "java.util.HashSet" => Some("set"),
        _ => None,
    }
}

impl<'t> JavaRule<'t> for SimplifiableStream {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["isPresent", "isEmpty", "count", "collect", "forEach"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) = ctx.call_name(node) else {
            return;
        };
        match name {
            "isPresent" | "isEmpty" => {
                if syntax::argument_count(node) == 0 {
                    Self::find_present(ctx, node, name);
                }
            }
            "count" => {
                if syntax::argument_count(node) == 0 {
                    Self::count_compared(ctx, node);
                }
            }
            "collect" => Self::collect_counting(ctx, node),
            "forEach" => Self::for_each(ctx, node),
            _ => {}
        }
    }
}
