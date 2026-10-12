//! `java/unguarded-log-argument` — `log.debug("state {}", state.toJson())` computes the
//! argument even when debug logging is off.

use super::util::{self, LogFamily};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Incubating,
    tags: &["logging", "performance"],
    remediation_minutes: 2,
    description: r#"Detects debug- and trace-level logging calls (`debug`, `trace`; `fine`,
`finer`, `finest` and `log(Level.FINE, …)` for `java.util.logging`) with an argument that does
real work — a conversion such as `toString()` or `toJson()`, a call that formats, renders, or
computes something, an object creation, a string concatenation — outside an
`isDebugEnabled()` / `isTraceEnabled()` / `isLoggable(..)` check.

**Why it matters.** Java evaluates every argument before the logger decides to discard the
message. Debug and trace output is normally off in production, so each such call serializes,
formats, copies, or allocates for nothing — and does it on every pass through the code.

**How to fix.** Pass the value itself and let the logger's `{}` placeholder call `toString()`
only when needed, pass a supplier (`() -> state.toJson()`) to an API that accepts one, or wrap
the call in a level check.

```java
// Noncompliant
log.debug("Request {} took {}", request.toJson(), Duration.ofMillis(elapsed));

// Compliant
if (log.isDebugEnabled()) {
    log.debug("Request {} took {}", request.toJson(), Duration.ofMillis(elapsed));
}
log.debug("Request {} took {} ms", request, elapsed);
```

Not reported: arguments that are variables, fields, literals, constants, lambdas, method
references, zero-argument reads (`getName()`, `size()`, a record accessor such as `id()`),
lookups with simple arguments (`map.get(key)`, `getProperty(NAME)`), `String.valueOf(..)` of
a primitive value, or `toString()` on a value of a known enum type (a JDK enum, or one
declared in the file that does not define its own `toString`), whose cost is negligible;
calls whose *message* is a concatenation or `String.format(..)` (reported by
`java/log-message-concatenation`); calls in a method that tests the level before them or under
a condition naming a debug/trace flag; calls in a `private` method that is only called from
such guarded places in the file (`if (log.isDebugEnabled()) { logDetails(store); }`); test
files; and loggers the file does not identify (the receiver's declared type must be a known
logging API, or a conventional logger name in a file importing one).

Spring's `LogAccessor` is covered too. It has no placeholders and no concatenation rule knows
it, so for it a message built by concatenation or `String.format(..)` is reported as well; the
fix is its `Supplier` overload (`log.debug(() -> "Loaded " + rows)`)."#,
    ..RuleMeta::new(
        "java/unguarded-log-argument",
        "java",
        "Debug log argument computed without a level check",
    )
};

#[derive(Default)]
pub struct UnguardedLogArgument<'t> {
    /// Findings inside `private` methods: reported in `finish` unless every call of the method
    /// in the file is guarded.
    parked: Vec<Parked<'t>>,
}

struct Parked<'t> {
    at: Node<'t>,
    message: String,
    method: Node<'t>,
}

const LEVELS: &[&str] = &["debug", "trace", "fine", "finer", "finest", "log"];

/// Name stems of zero-argument methods that build or transform something: `toString()`,
/// `toJson()`, `render()`, `describeAll()`. Every other zero-argument call (`getId()`,
/// `size()`, a record accessor such as `serverId()`) is taken as a cheap read.
const COSTLY_NULLARY: &[&str] = &[
    "to",
    "format",
    "render",
    "describe",
    "dump",
    "print",
    "serialize",
    "build",
    "compute",
    "calculate",
    "collect",
    "clone",
    "copy",
    "summarize",
    "summary",
    "join",
    "load",
    "read",
    "fetch",
    "query",
    "find",
    "list",
    "sort",
    "sorted",
    "encode",
    "decode",
    "digest",
    "export",
    "generate",
    "create",
    "parse",
    "evaluate",
    "scan",
    "search",
    "resolve",
    "convert",
];

/// Calls with arguments that are simple lookups or arithmetic.
const CHEAP_WITH_ARGS: &[&str] = &[
    "get",
    "getOrDefault",
    "containsKey",
    "contains",
    "charAt",
    "identityHashCode",
    "min",
    "max",
    "abs",
];

/// Whether `name` is `stem`, or `stem` followed by a capitalized word (`toJson` for `to`, but
/// not `total`).
fn has_stem(name: &str, stem: &str) -> bool {
    name.strip_prefix(stem)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(|c: char| c.is_ascii_uppercase()))
}

/// Whether `name` reads like a plain accessor or lookup: `getX`, `isX`, `hasX`.
fn is_accessor_name(name: &str) -> bool {
    ["get", "is", "has"].iter().any(|p| {
        name.strip_prefix(p)
            .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_uppercase()))
    })
}

/// `String.valueOf(x)` with `x` of a primitive type (`String.valueOf(flag)` returns a constant
/// string for a `boolean`, a short conversion otherwise).
fn is_string_value_of_primitive<'t>(ctx: &Ctx<'t>, call: Node<'t>, name: &str) -> bool {
    if name != "valueOf" || syntax::argument_count(call) != 1 {
        return false;
    }
    let on_string = call
        .field(Field::Object)
        .is_some_and(|o| ctx.type_name_of(o).as_deref() == Some("java.lang.String"));
    on_string
        && syntax::arguments(call)
            .next()
            .and_then(|a| ctx.type_of(a))
            .is_some_and(|t| t.is_primitive())
}

/// `x.toString()` where `x` is provably of an enum type that keeps `Enum.toString()` (it returns
/// the constant's name): a JDK enum, or an enum declared in this file whose declaration does not
/// mention `toString`.
fn is_enum_to_string<'t>(ctx: &Ctx<'t>, call: Node<'t>, name: &str) -> bool {
    if name != "toString" {
        return false;
    }
    let Some(t) = call.field(Field::Object).and_then(|o| ctx.type_of(o)) else {
        return false;
    };
    let Some(fqn) = t.fqn() else {
        return false;
    };
    if crate::resolve::is_enum_class(fqn) {
        return true;
    }
    if !ctx.is_subtype_of(&t, "java.lang.Enum") {
        return false;
    }
    find_type_decl(ctx, ctx.root(), fqn, 0).is_some_and(|d| !ctx.text(d).contains("toString"))
}

/// The type declaration of this file whose FQN is `fqn`. Visits type declarations and their
/// member types only (not method bodies).
fn find_type_decl<'t>(ctx: &Ctx<'t>, scope: Node<'t>, fqn: &str, depth: u32) -> Option<Node<'t>> {
    if depth > 8 {
        return None;
    }
    let members: Vec<Node<'t>> = if kind(scope) == "program" {
        named_children(scope).collect()
    } else {
        syntax::type_body(scope)
            .map(|b| ctx.members(b).collect())
            .unwrap_or_default()
    };
    members
        .into_iter()
        .filter(|m| syntax::is_type_declaration(*m))
        .find_map(|m| {
            if ctx.class_fqn(m).as_deref() == Some(fqn) {
                Some(m)
            } else {
                find_type_decl(ctx, m, fqn, depth + 1)
            }
        })
}

/// Whether a call is cheap: a zero-argument read or a lookup with cheap arguments, on a cheap
/// receiver.
fn is_cheap_call<'t>(ctx: &Ctx<'t>, call: Node<'t>, depth: u32) -> bool {
    if depth > 8 {
        return false;
    }
    let Some(name) = ctx.call_name(call) else {
        return false;
    };
    let cheap_name = if syntax::argument_count(call) == 0 {
        !COSTLY_NULLARY.iter().any(|s| has_stem(name, s)) || is_enum_to_string(ctx, call, name)
    } else {
        (CHEAP_WITH_ARGS.contains(&name)
            || is_accessor_name(name)
            || is_string_value_of_primitive(ctx, call, name))
            && syntax::arguments(call).all(|a| !does_work(ctx, a, depth + 1))
    };
    if !cheap_name {
        return false;
    }
    match call.field(Field::Object) {
        None => true,
        Some(o) => !does_work(ctx, o, depth + 1),
    }
}

/// Whether evaluating `e` does more than read variables and plain accessors.
fn does_work<'t>(ctx: &Ctx<'t>, e: Node<'t>, depth: u32) -> bool {
    if depth > 8 {
        return false;
    }
    let e = syntax::strip_parens(e);
    match kind(e) {
        "object_creation_expression" => true,
        "method_invocation" => !is_cheap_call(ctx, e, depth),
        "array_creation_expression" => e
            .field(Field::Value)
            .is_some_and(|init| named_children(init).any(|x| does_work(ctx, x, depth + 1))),
        "binary_expression" => {
            if syntax::operator_str(e) == "+" && is_dynamic_concat(ctx, e) {
                return true;
            }
            e.field(Field::Left)
                .is_some_and(|l| does_work(ctx, l, depth + 1))
                || e.field(Field::Right)
                    .is_some_and(|r| does_work(ctx, r, depth + 1))
        }
        "ternary_expression" => [Field::Condition, Field::Consequence, Field::Alternative]
            .iter()
            .filter_map(|f| e.field(*f))
            .any(|x| does_work(ctx, x, depth + 1)),
        "cast_expression" => e
            .field(Field::Value)
            .is_some_and(|v| does_work(ctx, v, depth + 1)),
        "field_access" => e
            .field(Field::Object)
            .is_some_and(|o| does_work(ctx, o, depth + 1)),
        "array_access" => {
            e.field(Field::Array)
                .is_some_and(|a| does_work(ctx, a, depth + 1))
                || e.field(Field::Index)
                    .is_some_and(|i| does_work(ctx, i, depth + 1))
        }
        "unary_expression" => e
            .field(Field::Operand)
            .is_some_and(|o| does_work(ctx, o, depth + 1)),
        _ => false,
    }
}

/// A `+` chain that builds a string (a string literal among the parts, or a `String` result)
/// with at least one part that is not a literal or a constant.
fn is_dynamic_concat<'t>(ctx: &Ctx<'t>, e: Node<'t>) -> bool {
    let mut has_string = false;
    let mut dynamic = false;
    let mut stack = vec![e];
    let mut steps = 0;
    while let Some(n) = stack.pop() {
        steps += 1;
        if steps > 256 {
            return false;
        }
        let n = syntax::strip_parens(n);
        if kind(n) == "binary_expression" && syntax::operator_str(n) == "+" {
            stack.extend(n.field(Field::Right));
            stack.extend(n.field(Field::Left));
            continue;
        }
        if kind(n) == "string_literal" {
            has_string = true;
        } else if !is_constant(ctx, n) {
            dynamic = true;
        }
    }
    dynamic && (has_string || ctx.type_of(e).is_some_and(|t| t.is_string()))
}

/// Literals and names that look like constants (`MAX`, `Type.NAME`).
fn is_constant<'t>(ctx: &Ctx<'t>, n: Node<'t>) -> bool {
    if util::is_literal(n) || kind(n) == "class_literal" {
        return true;
    }
    match kind(n) {
        "identifier" => util::is_constant_name(ctx.text(n)),
        "field_access" => n
            .field(Field::Field)
            .is_some_and(|f| util::is_constant_name(ctx.text(f))),
        _ => false,
    }
}

/// Whether the message argument is the territory of `java/log-message-concatenation`: a
/// concatenation, `String.format(..)` / `MessageFormat.format(..)`, or `"…".formatted(..)`.
fn is_built_message<'t>(ctx: &Ctx<'t>, msg: Node<'t>) -> bool {
    let m = syntax::strip_parens(msg);
    match kind(m) {
        "binary_expression" => syntax::operator_str(m) == "+",
        "method_invocation" => match ctx.call_name(m) {
            Some("format") => m.field(Field::Object).is_some_and(|o| {
                matches!(
                    ctx.type_name_of(o).as_deref(),
                    Some("java.lang.String" | "java.text.MessageFormat")
                )
            }),
            Some("formatted") => m
                .field(Field::Object)
                .is_some_and(|o| kind(o) == "string_literal"),
            _ => false,
        },
        _ => false,
    }
}

/// `Level.FINE` / `Level.FINER` / `Level.FINEST` (possibly qualified).
fn is_debug_jul_level<'t>(ctx: &Ctx<'t>, arg: Node<'t>) -> bool {
    let a = syntax::strip_parens(arg);
    let (obj, field) = match kind(a) {
        "field_access" => match (a.field(Field::Object), a.field(Field::Field)) {
            (Some(o), Some(f)) => (Some(o), f),
            _ => return false,
        },
        // `FINE` through a static import.
        "identifier" => (None, a),
        _ => return false,
    };
    if !matches!(ctx.text(field), "FINE" | "FINER" | "FINEST") {
        return false;
    }
    match obj {
        Some(o) => matches!(ctx.text(o), "Level" | "java.util.logging.Level"),
        None => ctx
            .imports()
            .iter()
            .any(|i| i.is_static && i.path.starts_with("java.util.logging.Level")),
    }
}

impl<'t> JavaRule<'t> for UnguardedLogArgument<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        LEVELS
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(level) = ctx.call_name(node) else {
            return;
        };
        let jul = match level {
            "debug" | "trace" => false,
            "fine" | "finer" | "finest" | "log" => true,
            _ => return,
        };
        if ctx.is_test() {
            return;
        }
        let Some(receiver) = node.field(Field::Object) else {
            return;
        };
        let mut args = syntax::arguments(node);
        if level == "log" {
            match args.next() {
                Some(l) if is_debug_jul_level(ctx, l) => {}
                _ => return,
            }
        }
        let Some(msg) = args.next() else {
            return;
        };
        let built = level != "log" && is_built_message(ctx, msg);
        let Some(costly) = std::iter::once(msg)
            .chain(args)
            .find(|a| does_work(ctx, *a, 0))
        else {
            return;
        };
        let Some(family) = util::logger_family(ctx, receiver) else {
            return;
        };
        if (family == LogFamily::Jul) != jul {
            return;
        }
        // A built message belongs to `java/log-message-concatenation`, except for Spring's
        // `LogAccessor`, which that rule does not know.
        if built && family != LogFamily::Supplier {
            return;
        }
        if util::log_call_is_guarded(ctx, node) {
            return;
        }
        let shown = util::shorten(ctx.text(costly), 40);
        let what = if level == "log" { "fine" } else { level };
        let message = if family == LogFamily::Supplier {
            format!(
                "Pass a supplier to this `{what}` call: `{shown}` is evaluated even when \
                 {what} logging is off"
            )
        } else {
            format!(
                "Guard this `{what}` call with a level check: `{shown}` is evaluated even when \
                 {what} logging is off"
            )
        };
        // In a private helper, the guard may sit at the call sites: decide once the whole
        // file has been seen.
        if let Some(method) = ctx
            .enclosing_method(node)
            .filter(|m| kind(*m) == "method_declaration" && syntax::has_modifier(*m, "private"))
        {
            self.parked.push(Parked {
                at: costly,
                message,
                method,
            });
            return;
        }
        ctx.emit(costly, message);
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        let mut verdicts: FxHashMap<usize, bool> = FxHashMap::default();
        for p in &self.parked {
            let guarded = *verdicts
                .entry(p.method.id())
                .or_insert_with(|| every_call_guarded(ctx, p.method));
            if !guarded {
                ctx.emit(p.at, p.message.clone());
            }
        }
    }
}

/// Whether the private method `method` is called at least once in this file and every call
/// (outside its own body) is guarded by a level check. A method reference (`this::helper`)
/// counts as an unguarded use. Occurrences of the name in comments and strings are ignored.
fn every_call_guarded<'t>(ctx: &Ctx<'t>, method: Node<'t>) -> bool {
    let Some(name_node) = method.field(Field::Name) else {
        return false;
    };
    let name = ctx.text(name_node);
    // A missing name (broken code): the search below would never advance.
    if name.is_empty() {
        return false;
    }
    let body = method.field(Field::Body);
    let src = ctx.source();
    let is_ident = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    let mut calls = 0;
    let mut from = 0;
    while let Some(start) = src.get(from..).and_then(|s| s.find(name)).map(|i| i + from) {
        let end = start + name.len();
        from = end;
        if src[..start].chars().next_back().is_some_and(is_ident)
            || src[end..].chars().next().is_some_and(is_ident)
        {
            continue;
        }
        if start == name_node.start_byte()
            || body.is_some_and(|b| b.start_byte() <= start && end <= b.end_byte())
        {
            continue;
        }
        let Some(ident) = ctx
            .root()
            .descendant_for_byte_range(start, end)
            .filter(|n| kind(*n) == "identifier")
        else {
            continue;
        };
        let Some(parent) = ctx.parent(ident) else {
            return false;
        };
        match kind(parent) {
            "method_invocation"
                if parent
                    .field(Field::Name)
                    .is_some_and(|n| n.id() == ident.id()) =>
            {
                if !inside_level_guard(ctx, parent) {
                    return false;
                }
                calls += 1;
            }
            "method_reference" => return false,
            _ => {}
        }
    }
    calls > 0
}

/// Whether `call` only runs when debug or trace logging is on: it is in the `then` branch of an
/// `if` (or ternary), or the right operand of `&&`, whose condition calls `isDebugEnabled()`,
/// `isTraceEnabled()` or `isLoggable(Level.FINE…)` without negating it.
fn inside_level_guard<'t>(ctx: &Ctx<'t>, call: Node<'t>) -> bool {
    let mut child = call;
    for a in ctx.ancestors(call) {
        let condition = match kind(a) {
            "if_statement" | "ternary_expression" => a
                .field(Field::Consequence)
                .filter(|c| c.id() == child.id())
                .and_then(|_| a.field(Field::Condition)),
            "binary_expression" if syntax::operator_str(a) == "&&" => a
                .field(Field::Right)
                .filter(|r| r.id() == child.id())
                .and_then(|_| a.field(Field::Left)),
            "method_declaration"
            | "constructor_declaration"
            | "lambda_expression"
            | "class_body" => return false,
            _ => None,
        };
        if condition.is_some_and(|c| is_level_check(ctx.text(c))) {
            return true;
        }
        child = a;
    }
    false
}

/// A condition testing that debug or trace output is on (and not its negation).
fn is_level_check(text: &str) -> bool {
    let checks = text.contains("isDebugEnabled")
        || text.contains("isTraceEnabled")
        || (text.contains("isLoggable") && text.contains("FINE"));
    let negated = text
        .char_indices()
        .any(|(i, c)| c == '!' && !text[i + 1..].starts_with('='));
    checks && !negated
}
