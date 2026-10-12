//! `java/log-message-concatenation` — `log.debug("x=" + x)` builds the message even when
//! debug logging is off.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["logging", "performance"],
    remediation_minutes: 2,
    description: r#"Detects debug- and trace-level logging calls (`debug`, `trace`; `fine`,
`finer`, `finest` for `java.util.logging`) whose message is built eagerly — by string
concatenation or `String.format` with non-constant parts — outside an `isDebugEnabled()` /
`isTraceEnabled()` / `isLoggable(..)` check, and precondition checks whose failure message is
built the same way although it is only needed when the check fails:
`Objects.requireNonNull(x, "no " + name)`, Guava's `checkArgument` / `checkState` /
`checkNotNull` / `verify`, Spring's `Assert` methods, and Commons Lang's `Validate` methods.

**Why it matters.** Debug and trace output is normally switched off in production, yet the
message is built before the logger gets a chance to discard it: every call pays for string
building, `toString()` calls, and garbage. On hot paths this cost is measurable.

The same holds for a precondition: the check almost always passes, yet its message is
concatenated every time.

**How to fix.** Use the logger's parameterized form (SLF4J and Log4j 2 `{}` placeholders,
`java.util.logging` `{0}` parameters or a `Supplier`), or wrap the call in a level check. For
preconditions, pass the message lazily: a `Supplier` (`requireNonNull(x, () -> "no " + name)`,
Spring's `Assert`) or template arguments (Guava's `checkArgument(ok, "bad %s", name)`,
`Validate.isTrue(ok, "bad %s", name)`).

```java
// Noncompliant
log.debug("Loaded " + rows.size() + " rows for " + table);

// Compliant
log.debug("Loaded {} rows for {}", rows.size(), table);
if (log.isDebugEnabled()) {
    log.debug("Loaded " + rows.size() + " rows for " + table);
}
```

The logger is recognized by its declared type (SLF4J, Log4j 1/2, Commons Logging, JBoss
Logging, `java.util.logging`) or, for inherited fields, by a conventional name (`log`,
`logger`, `LOG`, `LOGGER`) in a file importing a logging API. Precondition calls count when they
provably go to one of the classes above (qualified by it, or statically imported from it). Other
log levels (`info`, `warn`, `error`) are deliberately not reported: they are normally enabled,
so their message is needed anyway. Concatenations of literals and
constants are not reported, and neither is any call in a method that tests the level
(`isDebugEnabled`, `isTraceEnabled`, `isLoggable`, `isEnabled`) before it."#,
    ..RuleMeta::new(
        "java/log-message-concatenation",
        "java",
        "Eagerly built debug log or precondition message",
    )
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Family {
    /// SLF4J, Log4j 2, JBoss Logging: `{}` placeholders.
    Placeholders,
    /// Log4j 1, Commons Logging: no placeholders, guard the call.
    GuardOnly,
    /// `java.util.logging`.
    Jul,
}

fn family_of(fqn: &str) -> Option<Family> {
    match fqn {
        "org.slf4j.Logger"
        | "org.apache.logging.log4j.Logger"
        | "org.jboss.logging.Logger"
        | "org.apache.logging.log4j.core.Logger"
        | "ch.qos.logback.classic.Logger" => Some(Family::Placeholders),
        "org.apache.log4j.Logger"
        | "org.apache.log4j.Category"
        | "org.apache.commons.logging.Log" => Some(Family::GuardOnly),
        "java.util.logging.Logger" => Some(Family::Jul),
        _ => None,
    }
}

/// Logging packages whose import makes a conventional logger name trustworthy.
fn imported_family(ctx: &Ctx<'_>) -> Option<Family> {
    let mut found = None;
    for i in ctx.imports() {
        let p = i.path;
        let f = if p.starts_with("org.slf4j")
            || p.starts_with("org.apache.logging.log4j")
            || p.starts_with("org.jboss.logging")
            || p.starts_with("lombok.extern.slf4j")
            || p.starts_with("lombok.extern.log4j")
        {
            Family::Placeholders
        } else if p.starts_with("org.apache.log4j") || p.starts_with("org.apache.commons.logging") {
            Family::GuardOnly
        } else if p.starts_with("java.util.logging") {
            Family::Jul
        } else {
            continue;
        };
        if found.is_some_and(|x| x != f) {
            return None;
        }
        found = Some(f);
    }
    found
}

/// Debug-level method names (SLF4J/Log4j/Commons/JBoss, then `java.util.logging`).
const LEVELS: &[&str] = &["debug", "trace", "fine", "finer", "finest"];

const CONVENTIONAL: &[&str] = &["log", "logger", "LOG", "LOGGER", "Log", "Logger"];

const GUARDS: &[&str] = &[
    "isDebugEnabled",
    "isTraceEnabled",
    "isLoggable",
    "isEnabled",
    "isFineEnabled",
];

pub struct LogMessageConcatenation;

/// Precondition classes, the checks whose message is built eagerly, and how to pass it lazily.
const PRECONDITIONS: &[(&str, &[&str], &str)] = &[
    ("java.util.Objects", &["requireNonNull"], "a Supplier"),
    (
        "com.google.common.base.Preconditions",
        &["checkArgument", "checkState", "checkNotNull"],
        "template arguments",
    ),
    (
        "com.google.common.base.Verify",
        &["verify", "verifyNotNull"],
        "template arguments",
    ),
    (
        "org.springframework.util.Assert",
        &[
            "notNull",
            "isNull",
            "isTrue",
            "state",
            "hasText",
            "hasLength",
            "notEmpty",
            "noNullElements",
            "doesNotContain",
            "isInstanceOf",
            "isAssignable",
        ],
        "a Supplier",
    ),
    (
        "org.apache.commons.lang3.Validate",
        &[
            "notNull",
            "isTrue",
            "notEmpty",
            "notBlank",
            "validState",
            "noNullElements",
            "validIndex",
        ],
        "template arguments",
    ),
];

/// Every method name of [`PRECONDITIONS`].
const PRECONDITION_NAMES: &[&str] = &[
    "requireNonNull",
    "checkArgument",
    "checkState",
    "checkNotNull",
    "verify",
    "verifyNotNull",
    "notNull",
    "isNull",
    "isTrue",
    "state",
    "hasText",
    "hasLength",
    "notEmpty",
    "noNullElements",
    "doesNotContain",
    "isInstanceOf",
    "isAssignable",
    "notBlank",
    "validState",
    "validIndex",
];

/// Log levels and precondition names: the calls this rule looks at.
const CALL_NAMES: &[&str] = &[
    "debug",
    "trace",
    "fine",
    "finer",
    "finest",
    "requireNonNull",
    "checkArgument",
    "checkState",
    "checkNotNull",
    "verify",
    "verifyNotNull",
    "notNull",
    "isNull",
    "isTrue",
    "state",
    "hasText",
    "hasLength",
    "notEmpty",
    "noNullElements",
    "doesNotContain",
    "isInstanceOf",
    "isAssignable",
    "notBlank",
    "validState",
    "validIndex",
];

/// The precondition class a call provably goes to (qualified by the class, or statically
/// imported from it), with its fix advice.
fn precondition_owner<'t>(
    ctx: &Ctx<'t>,
    call: Node<'t>,
    name: &str,
) -> Option<(&'static str, &'static str)> {
    let owners = PRECONDITIONS
        .iter()
        .filter(|(_, names, _)| names.contains(&name));
    match util::fld(call, F.object) {
        Some(obj) => {
            if !matches!(kind(obj), "identifier" | "field_access") {
                return None;
            }
            // Cheap text test first: the receiver must spell an owner class (`Objects`,
            // `java.util.Objects`); resolving arbitrary receivers would index every method.
            let text = ctx.text(obj);
            if !owners
                .clone()
                .any(|(o, _, _)| *o == text || o.rsplit('.').next() == Some(text))
            {
                return None;
            }
            let fqn = ctx.type_name_of(obj)?;
            owners
                .into_iter()
                .find(|(o, _, _)| *o == fqn)
                .map(|(o, _, how)| (*o, *how))
        }
        None => {
            // A static import of the method (or of all members) from exactly one owner; a
            // method of this file with that name wins.
            let mut found = None;
            for i in ctx.imports().iter().filter(|i| i.is_static) {
                let owner = if i.is_wildcard {
                    i.path
                } else if i.simple_name() == name {
                    i.qualifier()
                } else {
                    continue;
                };
                match owners.clone().find(|(o, _, _)| *o == owner) {
                    Some((o, _, how)) => {
                        if found.is_some_and(|(f, _)| f != *o) {
                            return None;
                        }
                        found = Some((*o, *how));
                    }
                    None if !i.is_wildcard => return None,
                    None => {}
                }
            }
            if found.is_some()
                && ctx
                    .enclosing_class(call)
                    .and_then(syntax::type_body)
                    .is_some_and(|b| util::declares_method(ctx, b, name))
            {
                return None;
            }
            found
        }
    }
}

impl LogMessageConcatenation {
    fn logger_family<'t>(ctx: &Ctx<'t>, receiver: Node<'t>) -> Option<Family> {
        let ident = match kind(receiver) {
            "identifier" => receiver,
            "field_access" => {
                let obj = util::fld(receiver, F.object)?;
                if kind(obj) != "this" {
                    return None;
                }
                util::fld(receiver, F.field)?
            }
            _ => return None,
        };
        if let Some(decl) = ctx.declaration_of(ident) {
            let ty = decl.type_node?;
            let fqn = util::type_fqn(ctx, ty)?;
            return family_of(&fqn);
        }
        // Inherited (or Lombok-generated) logger field.
        if CONVENTIONAL.contains(&ctx.text(ident)) {
            return imported_family(ctx);
        }
        None
    }

    /// Whether the message is built at run time from non-constant parts.
    fn is_eager<'t>(ctx: &Ctx<'t>, msg: Node<'t>) -> bool {
        let msg = syntax::strip_parens(msg);
        match kind(msg) {
            "binary_expression" => {
                if syntax::operator_str(msg) != "+" {
                    return false;
                }
                // String concatenation with at least one non-constant operand.
                let mut has_string = false;
                let mut dynamic = false;
                let mut stack = vec![msg];
                let mut steps = 0;
                while let Some(n) = stack.pop() {
                    steps += 1;
                    if steps > 256 {
                        return false;
                    }
                    let n = syntax::strip_parens(n);
                    if kind(n) == "binary_expression" && syntax::operator_str(n) == "+" {
                        stack.extend(util::fld(n, F.right));
                        stack.extend(util::fld(n, F.left));
                        continue;
                    }
                    if syntax::is_string_literal(n) {
                        has_string = true;
                    } else if !util::is_constant_like(ctx, n) {
                        dynamic = true;
                    }
                }
                dynamic && (has_string || ctx.type_of(msg).is_some_and(|t| t.is_string()))
            }
            "method_invocation" => {
                let name = util::call_name(ctx, msg);
                match name {
                    "format" => {
                        let owner = util::fld(msg, F.object).and_then(|o| ctx.type_name_of(o));
                        matches!(
                            owner.as_deref(),
                            Some("java.lang.String" | "java.text.MessageFormat")
                        ) && util::args(msg)
                            .skip(1)
                            .any(|a| !util::is_constant_like(ctx, a))
                    }
                    "formatted" => {
                        util::fld(msg, F.object).is_some_and(syntax::is_string_literal)
                            && util::args(msg).any(|a| !util::is_constant_like(ctx, a))
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    /// `requireNonNull(x, "no " + name)` and friends: the message argument (the second one;
    /// the last one for Spring's `Assert`) is concatenated although it is only needed when
    /// the check fails.
    fn check_precondition<'t>(node: Node<'t>, ctx: &Ctx<'t>, name: &str) {
        let n = util::arg_count(node);
        if n < 2 {
            return;
        }
        // Cheap syntactic test before any resolution.
        let spring_index = n - 1;
        let candidates = [1, spring_index];
        if !candidates.iter().any(|i| {
            util::arg(node, *i).is_some_and(|m| {
                matches!(
                    kind(syntax::strip_parens(m)),
                    "binary_expression" | "method_invocation"
                )
            })
        }) {
            return;
        }
        let Some((owner, how)) = precondition_owner(ctx, node, name) else {
            return;
        };
        let index = if owner == "org.springframework.util.Assert" {
            spring_index
        } else {
            1
        };
        let Some(msg) = util::arg(node, index) else {
            return;
        };
        if !Self::is_eager(ctx, msg) {
            return;
        }
        ctx.emit(
            msg,
            format!(
                "Pass {how} instead of building the message of `{name}` eagerly: it is built \
                 even when the check passes"
            ),
        );
    }

    /// Whether the enclosing function tests the log level before the call.
    fn is_guarded<'t>(ctx: &Ctx<'t>, call: Node<'t>) -> bool {
        let Some(func) = ctx
            .ancestors(call)
            .find(|a| syntax::is_method_like(*a) || kind(*a) == "static_initializer")
            .or_else(|| ctx.enclosing_function(call))
        else {
            return false;
        };
        let src = ctx.source();
        let text = src.get(func.start_byte()..call.start_byte()).unwrap_or("");
        GUARDS.iter().any(|g| text.contains(g))
    }
}

impl<'t> JavaRule<'t> for LogMessageConcatenation {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        CALL_NAMES
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if let Some(name) = ctx.call_name(node) {
            if PRECONDITION_NAMES.contains(&name) {
                Self::check_precondition(node, ctx, name);
                return;
            }
        }
        let Some(level) = util::call_named_with(ctx, node, &["debug", "trace", "fine"], LEVELS)
        else {
            return;
        };
        let jul_level = matches!(level, "fine" | "finer" | "finest");
        if !jul_level && !matches!(level, "debug" | "trace") {
            return;
        }
        let Some(msg) = util::arg(node, 0) else {
            return;
        };
        if !matches!(
            kind(syntax::strip_parens(msg)),
            "binary_expression" | "method_invocation"
        ) {
            return;
        }
        let Some(receiver) = util::fld(node, F.object) else {
            return;
        };
        if !Self::is_eager(ctx, msg) {
            return;
        }
        let Some(family) = Self::logger_family(ctx, receiver) else {
            return;
        };
        if (family == Family::Jul) != jul_level {
            return;
        }
        if Self::is_guarded(ctx, node) {
            return;
        }
        let text = match family {
            Family::Placeholders => format!(
                "Use `{{}}` placeholders instead of building this `{level}` message, or guard \
                 the call with a level check"
            ),
            Family::GuardOnly => format!(
                "Guard this `{level}` call with a level check: the message is built even when \
                 the level is disabled"
            ),
            Family::Jul => format!(
                "Pass a `Supplier` or `{{0}}` parameters instead of building this `{level}` \
                 message, or guard the call with `isLoggable`"
            ),
        };
        ctx.emit(msg, text);
    }
}
