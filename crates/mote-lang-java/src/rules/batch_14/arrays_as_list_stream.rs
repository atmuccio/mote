//! `java/arrays-as-list-stream` — `Arrays.asList(..).stream()` where `Arrays.stream` or
//! `Stream.of` says it directly.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["streams", "idiom"],
    remediation_minutes: 2,
    description: r#"Detects `Arrays.asList(..).stream()`: a list is built only to be streamed.

**Why it matters.** `Arrays.stream(array)` and `Stream.of(a, b, c)` state the intent in one
call and skip the list wrapper. With a primitive array the long form is also wrong:
`Arrays.asList(intArray)` does not box the elements but makes a one-element `List<int[]>`,
so the stream holds the array itself, not its numbers. `Arrays.stream(intArray)` gives an
`IntStream` of the values.

**How to fix.** Stream the array with `Arrays.stream(array)`; stream separate values with
`Stream.of(..)`.

```java
// Noncompliant
long n = Arrays.asList(names).stream().filter(String::isBlank).count();
Stream<String> s = Arrays.asList("a", "b").stream();

// Compliant
long n = Arrays.stream(names).filter(String::isBlank).count();
Stream<String> s = Stream.of("a", "b");
```

Only `java.util.Arrays.asList` (qualified or statically imported) is considered. Not reported:
a single argument whose type the resolver does not know (it may be an array or a value),
`asList()` without arguments, and `parallelStream()`."#,
    ..RuleMeta::new(
        "java/arrays-as-list-stream",
        "java",
        "Arrays.asList(..).stream() instead of Arrays.stream or Stream.of",
    )
};

pub struct ArraysAsListStream;

/// Whether `call` is `Arrays.asList(..)` of `java.util.Arrays`.
fn is_as_list<'t>(ctx: &Ctx<'t>, call: Node<'t>) -> bool {
    if kind(call) != "method_invocation" || ctx.call_name(call) != Some("asList") {
        return false;
    }
    match call.field(Field::Object) {
        Some(recv) => util::names_type(ctx, recv, "java.util.Arrays"),
        None => ctx.imports().iter().any(|i| {
            i.is_static
                && ((i.is_wildcard && i.path == "java.util.Arrays")
                    || (!i.is_wildcard && i.path == "java.util.Arrays.asList"))
        }),
    }
}

impl<'t> JavaRule<'t> for ArraysAsListStream {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["stream"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if ctx.call_name(node) != Some("stream") || syntax::argument_count(node) != 0 {
            return;
        }
        let Some(list) = node.field(Field::Object).map(syntax::strip_parens) else {
            return;
        };
        if !is_as_list(ctx, list) {
            return;
        }
        let n = util::arg_count(list);
        let msg = match n {
            0 => return,
            1 => {
                let Some(arg) = util::arg(list, 0) else {
                    return;
                };
                let Some(t) = ctx.type_of(arg) else {
                    return;
                };
                if t.is_null() {
                    return;
                }
                if !t.is_array() {
                    "Use `Stream.of(..)` instead of building a list only to stream it"
                } else if t.element_type().is_some_and(JType::is_primitive) {
                    // An unknown element type is a class or type variable: never primitive.
                    "Use `Arrays.stream(..)`: `Arrays.asList` of a primitive array makes a list whose only element is the array itself"
                } else {
                    "Stream the array with `Arrays.stream(..)` instead of wrapping it in a list first"
                }
            }
            _ => "Use `Stream.of(..)` instead of building a list only to stream it",
        };
        ctx.emit(list, msg);
    }
}
