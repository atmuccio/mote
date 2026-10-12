//! Helpers shared by the batch-6 rules. Nothing here walks the whole tree: every function looks
//! at one expression, a node's ancestors, or the text of the enclosing method.

use crate::prelude::*;

// ------------------------------------------------------------------------- loggers

/// Logging API families, by how a lazy message is written.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum LogFamily {
    /// SLF4J, Log4j 2, JBoss Logging, Logback: `{}` placeholders.
    Placeholders,
    /// Log4j 1, Commons Logging: no placeholders, guard the call.
    GuardOnly,
    /// `java.util.logging`.
    Jul,
    /// Spring's `LogAccessor`: no placeholders, `Supplier` overloads.
    Supplier,
}

fn family_of_fqn(fqn: &str) -> Option<LogFamily> {
    match fqn {
        "org.slf4j.Logger"
        | "org.apache.logging.log4j.Logger"
        | "org.jboss.logging.Logger"
        | "org.apache.logging.log4j.core.Logger"
        | "ch.qos.logback.classic.Logger" => Some(LogFamily::Placeholders),
        "org.apache.log4j.Logger"
        | "org.apache.log4j.Category"
        | "org.apache.commons.logging.Log" => Some(LogFamily::GuardOnly),
        "java.util.logging.Logger" => Some(LogFamily::Jul),
        "org.springframework.core.log.LogAccessor" => Some(LogFamily::Supplier),
        _ => None,
    }
}

/// The single logging family whose package the file imports (`None` when none or several).
fn imported_family(ctx: &Ctx<'_>) -> Option<LogFamily> {
    let mut found = None;
    for i in ctx.imports() {
        let p = i.path;
        let f = if p.starts_with("org.slf4j")
            || p.starts_with("org.apache.logging.log4j")
            || p.starts_with("org.jboss.logging")
            || p.starts_with("lombok.extern.slf4j")
            || p.starts_with("lombok.extern.log4j")
        {
            LogFamily::Placeholders
        } else if p.starts_with("org.apache.log4j") || p.starts_with("org.apache.commons.logging") {
            LogFamily::GuardOnly
        } else if p.starts_with("java.util.logging") {
            LogFamily::Jul
        } else if p == "org.springframework.core.log.LogAccessor" {
            LogFamily::Supplier
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

/// Conventional names of logger fields (trusted for inherited or generated fields only when
/// the file imports a logging API).
const CONVENTIONAL_LOGGERS: &[&str] = &["log", "logger", "LOG", "LOGGER", "Log", "Logger"];

/// The logging family of a call receiver: a variable or field whose declared type is a known
/// logger, or a conventional logger name in a file importing one logging API.
pub(super) fn logger_family<'t>(ctx: &Ctx<'t>, receiver: Node<'t>) -> Option<LogFamily> {
    let ident = match kind(receiver) {
        "identifier" => receiver,
        "field_access" => {
            let obj = receiver.field(Field::Object)?;
            if kind(obj) != "this" {
                return None;
            }
            receiver.field(Field::Field)?
        }
        _ => return None,
    };
    if let Some(decl) = ctx.declaration_of(ident) {
        let ty = decl.type_node?;
        let base = if kind(ty) == "generic_type" {
            syntax::first_named_child(ty).unwrap_or(ty)
        } else {
            ty
        };
        if !matches!(kind(base), "type_identifier" | "scoped_type_identifier") {
            return None;
        }
        let fqn = ctx.resolve_type_name(ctx.text(base), base)?;
        return family_of_fqn(&fqn);
    }
    if CONVENTIONAL_LOGGERS.contains(&ctx.text(ident)) {
        return imported_family(ctx);
    }
    None
}

/// Level-check method names; one of them before the call in the same method means the author
/// already thought about the level.
const LEVEL_CHECKS: &[&str] = &[
    "isDebugEnabled",
    "isTraceEnabled",
    "isLoggable",
    "isEnabled",
    "isFineEnabled",
];

/// Whether the log call is guarded: the enclosing method tests the level somewhere before the
/// call, or an enclosing condition mentions a debug/trace flag (`if (DEBUG)`,
/// `if (traceOn && …)`).
pub(super) fn log_call_is_guarded<'t>(ctx: &Ctx<'t>, call: Node<'t>) -> bool {
    let func = ctx
        .ancestors(call)
        .find(|a| syntax::is_method_like(*a) || kind(*a) == "static_initializer")
        .or_else(|| ctx.enclosing_function(call));
    if let Some(func) = func {
        let text = ctx
            .source()
            .get(func.start_byte()..call.start_byte())
            .unwrap_or("");
        if LEVEL_CHECKS.iter().any(|g| text.contains(g)) {
            return true;
        }
    }
    let mut child = call;
    for a in ctx.ancestors(call) {
        let cond = match kind(a) {
            "if_statement" | "ternary_expression" => {
                a.field(Field::Condition).filter(|c| c.id() != child.id())
            }
            "binary_expression" if syntax::operator_str(a) == "&&" => {
                a.field(Field::Left).filter(|l| l.id() != child.id())
            }
            "method_declaration"
            | "constructor_declaration"
            | "lambda_expression"
            | "class_body" => return false,
            _ => None,
        };
        if cond.is_some_and(|c| mentions_level_flag(ctx.text(c))) {
            return true;
        }
        child = a;
    }
    false
}

fn mentions_level_flag(text: &str) -> bool {
    let b = text.as_bytes();
    ["debug", "trace", "fine"].iter().any(|w| {
        b.windows(w.len())
            .any(|win| win.eq_ignore_ascii_case(w.as_bytes()))
    })
}

// ------------------------------------------------------------------------- literals

/// Whether `n` (parentheses stripped) is a literal of any kind.
pub(super) fn is_literal(n: Node<'_>) -> bool {
    matches!(
        kind(n),
        "string_literal"
            | "character_literal"
            | "decimal_integer_literal"
            | "hex_integer_literal"
            | "octal_integer_literal"
            | "binary_integer_literal"
            | "decimal_floating_point_literal"
            | "hex_floating_point_literal"
            | "true"
            | "false"
            | "null_literal"
    )
}

/// Whether a name looks like a constant (`MAX_SIZE`, `DEFAULT`).
pub(super) fn is_constant_name(name: &str) -> bool {
    name.bytes().any(|c| c.is_ascii_uppercase())
        && name
            .bytes()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'_')
}

/// Shortens `text` to at most `max` characters (on a char boundary), with an ellipsis, and
/// collapses line breaks.
pub(super) fn shorten(text: &str, max: usize) -> String {
    let one_line: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() <= max {
        return one_line;
    }
    let mut s: String = one_line.chars().take(max.saturating_sub(1)).collect();
    s.push('…');
    s
}
