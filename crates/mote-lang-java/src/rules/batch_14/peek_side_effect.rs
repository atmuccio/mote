//! `java/peek-side-effect` — `Stream.peek` used for work other than logging or printing.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["streams", "side-effects"],
    remediation_minutes: 10,
    description: r#"Detects `peek(..)` on a `java.util.stream` pipeline whose action does real work —
changes an element or some other state, collects into a list, validates, counts — instead of
only logging or printing what flows by.

**Why it matters.** A stream runs `peek`'s action only for the elements the terminal operation
actually pulls through the pipeline, and the library is free to skip it altogether. Since
Java 9, `list.stream().peek(action).count()` does not run the action at all, because the count
is known from the source; `findFirst`, `anyMatch` or `limit` stop after a few elements; a
parallel stream runs the action on several threads in any order. Code that relies on the
action happening works until one of these conditions changes, then fails quietly. The API
itself describes `peek` as a debugging aid.

**How to fix.** Do the work where the stream guarantees it runs: in `map` (returning the
element), in the terminal `forEach`, or in a separate loop before or after the pipeline.

```java
// Noncompliant
List<Order> sent = orders.stream()
        .filter(Order::isReady)
        .peek(o -> o.setStatus(SENT))
        .collect(Collectors.toList());

// Compliant
List<Order> sent = orders.stream()
        .filter(Order::isReady)
        .collect(Collectors.toList());
sent.forEach(o -> o.setStatus(SENT));
```

Only pipelines that start with a `stream()` / `parallelStream()` call or that the resolver
proves to be `java.util.stream` streams are considered. Not reported: actions that only print
(`System.out::println`, `print`, `printf`) or log (`log.debug(..)`, also behind an
`isDebugEnabled()` test), empty actions, and actions passed as a variable (their body is not
visible)."#,
    ..RuleMeta::new(
        "java/peek-side-effect",
        "java",
        "Stream.peek used for side effects",
    )
};

pub struct PeekSideEffect;

/// Stream operations returning a stream of the same pipeline.
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
    "parallel",
    "unordered",
    "onClose",
];

/// Method names that print.
const PRINT: &[&str] = &["print", "println", "printf"];

/// Method names of logging levels (SLF4J, Log4j, commons-logging, JUL, System.Logger).
const LOG_LEVELS: &[&str] = &[
    "trace", "debug", "info", "warn", "warning", "error", "fatal", "fine", "finer", "finest",
    "config", "severe", "log",
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

/// Whether a receiver text names a logger (`log`, `LOG`, `logger`, `this.log`, `Log`).
fn looks_like_logger(text: &str) -> bool {
    let last = text.rsplit('.').next().unwrap_or(text);
    last.to_ascii_lowercase().contains("log")
}

/// Whether a method name on a receiver prints or logs.
fn is_output(ctx: &Ctx<'_>, name: &str, receiver: Option<Node<'_>>) -> bool {
    if PRINT.contains(&name) {
        return true;
    }
    LOG_LEVELS.contains(&name) && receiver.is_some_and(|r| looks_like_logger(ctx.text(r)))
}

/// Whether a call prints or logs.
fn is_output_call<'t>(ctx: &Ctx<'t>, e: Node<'t>) -> bool {
    let e = syntax::strip_parens(e);
    kind(e) == "method_invocation"
        && ctx
            .call_name(e)
            .is_some_and(|n| is_output(ctx, n, e.field(Field::Object)))
}

/// Whether a statement of a lambda body only prints or logs (possibly behind an `if`).
fn is_output_statement<'t>(ctx: &Ctx<'t>, s: Node<'t>, depth: u32) -> bool {
    if depth > 8 {
        return false;
    }
    match kind(s) {
        "expression_statement" => {
            syntax::first_named_child(s).is_some_and(|e| is_output_call(ctx, e))
        }
        "block" => named_children(s).all(|c| is_output_statement(ctx, c, depth + 1)),
        "if_statement" => {
            s.field(Field::Consequence)
                .is_some_and(|c| is_output_statement(ctx, c, depth + 1))
                && s.field(Field::Alternative)
                    .is_none_or(|a| is_output_statement(ctx, a, depth + 1))
        }
        "line_comment" | "block_comment" | "empty_statement" => true,
        _ => false,
    }
}

/// Whether the action does something other than printing or logging; `None` when its body is
/// not visible.
fn does_work<'t>(ctx: &Ctx<'t>, action: Node<'t>) -> Option<bool> {
    let action = syntax::strip_parens(action);
    match kind(action) {
        "method_reference" => {
            // The method name is the last named child (`System.out::println`).
            let name = named_children(action).last()?;
            if kind(name) != "identifier" {
                // `Foo::new` builds objects: work.
                return Some(true);
            }
            let receiver = syntax::first_named_child(action);
            Some(!is_output(ctx, ctx.text(name), receiver))
        }
        "lambda_expression" => {
            let body = action.field(Field::Body)?;
            if kind(body) == "block" {
                Some(!is_output_statement(ctx, body, 0))
            } else {
                Some(!is_output_call(ctx, body))
            }
        }
        _ => None,
    }
}

impl<'t> JavaRule<'t> for PeekSideEffect {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["peek"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if ctx.call_name(node) != Some("peek") || syntax::argument_count(node) != 1 {
            return;
        }
        let Some(action) = syntax::arguments(node).next() else {
            return;
        };
        if does_work(ctx, action) != Some(true) {
            return;
        }
        let (Some(obj), Some(name)) = (node.field(Field::Object), node.field(Field::Name)) else {
            return;
        };
        if !is_stream(ctx, obj, 0) {
            return;
        }
        ctx.emit(
            name,
            "Move this work out of `peek()`: the stream runs its action only for the elements it pulls, and may skip it entirely",
        );
    }
}
