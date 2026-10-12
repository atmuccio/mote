//! `java/format-argument-mismatch` — placeholders and arguments of a format call don't match.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["format", "logging"],
    remediation_minutes: 5,
    description: r#"Detects a format call whose string-literal pattern expects a different number of
arguments than it is given:

* `String.format`, `"…".formatted(..)`, `PrintStream`/`PrintWriter` `printf` and `format`
  (`%s`, `%d`, … — `%%` and `%n` take no argument);
* `MessageFormat.format` (`{0}`, `{1}`, … — the highest index decides);
* SLF4J and Log4j 2 logger calls (`log.info("a {} b {}", x, y)`), where a final exception
  argument beyond the placeholders is allowed (it is logged with its stack trace).

**Why it matters.** A missing argument makes `String.format` throw
`MissingFormatArgumentException` at run time — typically in an error path, hiding the
original problem. Extra arguments are silently ignored, and logger messages print a literal
`{}`: in both cases the message lacks the information the author wanted to record.

**How to fix.** Make the placeholders and the arguments agree.

```java
// Noncompliant
String.format("%s: %s", user);
log.warn("Retry {} of {} failed", attempt);

// Compliant
String.format("%s: %s", user, reason);
log.warn("Retry {} of {} failed", attempt, max);
```

Not reported: patterns that are not string literals, explicit argument indexes (`%2$s`,
`%<s`), a single argument that may be an `Object[]` passed as the varargs array, logger
calls whose first argument is not the message (markers), and MessageFormat patterns with
quotes."#,
    ..RuleMeta::new(
        "java/format-argument-mismatch",
        "java",
        "Format placeholders and arguments don't match",
    )
};

#[derive(Default)]
pub struct FormatArgumentMismatch {
    /// Whether the file mentions a logging framework whose messages use `{}` placeholders.
    logger_file: Option<bool>,
    /// Whether the file contains any call this rule looks at.
    any_call: util::Gate,
}

/// Number of arguments a `java.util.Formatter` pattern consumes; `None` for explicit indexes,
/// relative indexes, or anything malformed.
fn formatter_arg_count(fmt: &str) -> Option<usize> {
    let b = fmt.as_bytes();
    let mut i = 0;
    let mut count = 0;
    while i < b.len() {
        if b[i] != b'%' {
            i += 1;
            continue;
        }
        i += 1;
        // Explicit index `%2$s`.
        let digits = b[i..].iter().take_while(|c| c.is_ascii_digit()).count();
        if digits > 0 && b.get(i + digits) == Some(&b'$') {
            return None;
        }
        while let Some(&c) = b.get(i) {
            match c {
                b'<' => return None,
                b'-' | b'#' | b'+' | b' ' | b'0' | b',' | b'(' => i += 1,
                _ => break,
            }
        }
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if b.get(i) == Some(&b'.') {
            i += 1;
            if !b.get(i).is_some_and(u8::is_ascii_digit) {
                return None;
            }
            while b.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
        }
        match b.get(i)? {
            b'%' | b'n' => {}
            b't' | b'T' => {
                if !b.get(i + 1).is_some_and(u8::is_ascii_alphabetic) {
                    return None;
                }
                i += 1;
                count += 1;
            }
            b'b' | b'B' | b'h' | b'H' | b's' | b'S' | b'c' | b'C' | b'd' | b'o' | b'x' | b'X'
            | b'e' | b'E' | b'f' | b'g' | b'G' | b'a' | b'A' => count += 1,
            _ => return None,
        }
        i += 1;
    }
    Some(count)
}

/// Number of arguments a `MessageFormat` pattern needs (highest index + 1); `None` when the
/// pattern contains quotes or is malformed.
fn message_format_arg_count(fmt: &str) -> Option<usize> {
    if fmt.contains('\'') {
        return None;
    }
    let b = fmt.as_bytes();
    let mut i = 0;
    let mut needed = 0usize;
    while i < b.len() {
        if b[i] != b'{' {
            i += 1;
            continue;
        }
        i += 1;
        while b.get(i).is_some_and(u8::is_ascii_whitespace) {
            i += 1;
        }
        let digits_start = i;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == digits_start || i - digits_start > 4 {
            return None;
        }
        let index: usize = fmt.get(digits_start..i)?.parse().ok()?;
        needed = needed.max(index + 1);
        // Skip `,type,style}` with nested braces (choice formats).
        let mut depth = 1u32;
        while depth > 0 {
            match b.get(i)? {
                b'{' => depth += 1,
                b'}' => depth -= 1,
                _ => {}
            }
            i += 1;
        }
    }
    Some(needed)
}

/// Number of `{}` placeholders in an SLF4J / Log4j 2 message (`\{}` is a literal).
fn logger_placeholder_count(msg: &str) -> usize {
    let b = msg.as_bytes();
    let mut count = 0;
    let mut i = 0;
    while i + 1 < b.len() {
        if b[i] == b'{' && b[i + 1] == b'}' {
            let escaped = i >= 1 && b[i - 1] == b'\\';
            let double = i >= 2 && b[i - 2] == b'\\';
            if !escaped || double {
                count += 1;
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    count
}

/// Whether a single trailing argument might be an `Object[]` spread as the varargs array.
fn may_be_varargs_array<'t>(ctx: &Ctx<'t>, arg: Node<'t>) -> bool {
    match ctx.type_of(arg) {
        None => true,
        Some(t) => {
            t.is_null() || t.element_type().is_none_or(|e| !e.is_primitive()) && t.is_array()
        }
    }
}

const THROWABLE_NAMES: &[&str] = &[
    "e",
    "ex",
    "exc",
    "t",
    "th",
    "throwable",
    "exception",
    "cause",
    "err",
    "error",
    "ioe",
];

/// `Some(true)`: provably a `Throwable` (or a caught exception); `Some(false)`: provably not;
/// `None`: unknown.
fn is_throwable<'t>(ctx: &Ctx<'t>, arg: Node<'t>) -> Option<bool> {
    let arg = syntax::strip_parens(arg);
    if kind(arg) == "identifier" {
        if let Some(d) = ctx.declaration_of(arg) {
            if d.kind == VarKind::CatchParameter {
                return Some(true);
            }
        }
    }
    if let Some(t) = ctx.type_of(arg) {
        if let Some(b) = ctx.subtype(&t, "java.lang.Throwable") {
            return Some(b);
        }
    }
    if kind(arg) == "identifier" && THROWABLE_NAMES.contains(&ctx.text(arg)) {
        return Some(true);
    }
    None
}

/// Whether the logger receiver is an SLF4J or Log4j 2 logger (declared type, or Lombok's
/// generated `log` field).
fn is_placeholder_logger<'t>(ctx: &Ctx<'t>, recv: Node<'t>) -> bool {
    const LOGGERS: &[&str] = &["org.slf4j.Logger", "org.apache.logging.log4j.Logger"];
    let ident = match kind(recv) {
        "identifier" => recv,
        "field_access" => match (util::fld(recv, F::Object), util::fld(recv, F::Field)) {
            (Some(o), Some(f)) if kind(o) == "this" => f,
            _ => return false,
        },
        _ => return false,
    };
    if let Some(d) = ctx.declaration_of(ident) {
        return d
            .type_node
            .and_then(|t| util::type_fqn(ctx, t))
            .is_some_and(|f| LOGGERS.contains(&f.as_str()));
    }
    // Lombok `@Slf4j` / `@Log4j2` generate `log`.
    if ctx.text(ident) != "log" {
        return false;
    }
    const LOMBOK: &[&str] = &["lombok.extern.slf4j.Slf4j", "lombok.extern.log4j.Log4j2"];
    ctx.ancestors(recv)
        .filter(|a| syntax::is_type_declaration(*a))
        .any(|c| {
            syntax::annotations(c).any(|a| {
                matches!(syntax::annotation_name(a, ctx.source()), "Slf4j" | "Log4j2")
                    && util::annotation_is(ctx, a, LOMBOK)
            })
        })
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("{n} {one}")
    } else {
        format!("{n} {many}")
    }
}

impl FormatArgumentMismatch {
    /// `String.format`, `formatted`, `printf`, `PrintStream.format`, `MessageFormat.format`.
    fn check_format<'t>(&self, ctx: &Ctx<'t>, call: Node<'t>, name_node: Node<'t>, name: &str) {
        let Some(recv) = util::fld(call, F::Object) else {
            return;
        };
        let args: Vec<Node<'t>> = util::args(call).collect();
        // Where the pattern literal is (syntax only; types are checked on a mismatch).
        let (pattern, rest, locale_first): (Node<'t>, &[Node<'t>], bool) = if name == "formatted" {
            if kind(recv) != "string_literal" {
                return;
            }
            (recv, &args[..], false)
        } else {
            match (args.first(), args.get(1)) {
                (Some(&first), _) if syntax::is_string_literal(first) => (first, &args[1..], false),
                (Some(_), Some(&second)) if syntax::is_string_literal(second) => {
                    (second, &args[2..], true)
                }
                _ => return,
            }
        };
        let message_format = name == "format"
            && !locale_first
            && matches!(ctx.text(recv), "MessageFormat" | "java.text.MessageFormat");
        let Some(text) = util::string_value_loose(ctx, pattern) else {
            return;
        };
        let expected = if message_format {
            message_format_arg_count(&text)
        } else {
            formatter_arg_count(&text)
        };
        let Some(expected) = expected else {
            return;
        };
        let given = rest.len();
        if given == expected {
            return;
        }
        // A mismatch: now make sure this really is a format call.
        let receiver_ok = if name == "formatted" {
            true
        } else if message_format {
            util::names_type(ctx, recv, "java.text.MessageFormat")
        } else if name == "format" && util::names_type(ctx, recv, "java.lang.String") {
            true
        } else {
            kind(recv) != "string_literal"
                && ctx.type_of(recv).is_some_and(|t| {
                    ctx.is_subtype_of(&t, "java.io.PrintStream")
                        || ctx.is_subtype_of(&t, "java.io.PrintWriter")
                })
        };
        if !receiver_ok {
            return;
        }
        if locale_first
            && !args
                .first()
                .and_then(|a| ctx.type_of(*a))
                .is_some_and(|t| t.is_class("java.util.Locale"))
        {
            return;
        }
        if given == 1 && may_be_varargs_array(ctx, rest[0]) {
            return;
        }
        ctx.emit(
            name_node,
            format!(
                "Pass {} to match this format string, not {}",
                plural(expected, "argument", "arguments"),
                given
            ),
        )
        .value(expected as f64);
    }

    /// SLF4J / Log4j 2 `{}` messages.
    fn check_logger<'t>(&mut self, ctx: &Ctx<'t>, call: Node<'t>, name_node: Node<'t>) {
        let Some(recv) = util::fld(call, F::Object) else {
            return;
        };
        let src = ctx.source();
        let loggers = *self
            .logger_file
            .get_or_insert_with(|| src.contains("slf4j") || src.contains("log4j"));
        if !loggers {
            return;
        }
        let Some(msg) = util::first_arg(call) else {
            return;
        };
        if !syntax::is_string_literal(msg) {
            return;
        }
        let given = util::arg_count(call).saturating_sub(1);
        let raw = ctx.text(msg);
        // Nothing to compare in the common "message only" call without braces.
        if given == 0 && !raw.contains("{}") {
            return;
        }
        // Escapes only matter next to a placeholder; count on the source text when there are none.
        let text: std::borrow::Cow<'t, str> = if raw.contains('\\') {
            match util::string_value_loose(ctx, msg) {
                Some(t) => t.into(),
                None => return,
            }
        } else {
            raw.into()
        };
        let placeholders = logger_placeholder_count(&text);
        if given == placeholders {
            return;
        }
        let args: Vec<Node<'t>> = util::args(call).collect();
        let rest = &args[1..];
        if !is_placeholder_logger(ctx, recv) {
            return;
        }
        if given == placeholders + 1 {
            // A final exception is logged with its stack trace, not formatted.
            match rest.last().map(|a| is_throwable(ctx, *a)) {
                Some(Some(false)) => {}
                _ => return,
            }
        }
        if given == 1 && placeholders > 1 && may_be_varargs_array(ctx, rest[0]) {
            return;
        }
        let printf_style =
            placeholders == 0 && formatter_arg_count(&text).is_some_and(|n| n > 0 && n <= given);
        let message = if printf_style {
            "Use `{}` placeholders in this log message: the logger does not substitute printf-style `%` specifiers".to_string()
        } else {
            format!(
                "Make the placeholders and arguments of this log message agree: {} but {}",
                plural(placeholders, "placeholder", "placeholders"),
                plural(given, "argument", "arguments")
            )
        };
        ctx.emit(name_node, message);
    }
}

impl<'t> JavaRule<'t> for FormatArgumentMismatch {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &[
            "format",
            "formatted",
            "printf",
            "trace",
            "debug",
            "info",
            "warn",
            "error",
            "fatal",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !self
            .any_call
            .open_any(ctx, &["format(", "formatted(", "printf(", "slf4j", "log4j"])
        {
            return;
        }
        let Some(name_node) = util::fld(node, F::Name) else {
            return;
        };
        let name = ctx.text(name_node);
        match name {
            "format" | "formatted" | "printf" => self.check_format(ctx, node, name_node, name),
            "trace" | "debug" | "info" | "warn" | "error" | "fatal" => {
                self.check_logger(ctx, node, name_node)
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_formatter_arguments() {
        assert_eq!(formatter_arg_count("plain"), Some(0));
        assert_eq!(formatter_arg_count("%s and %d%%%n"), Some(2));
        assert_eq!(formatter_arg_count("%-10s|%05.2f|%,d|%tY"), Some(4));
        assert_eq!(formatter_arg_count("%2$s %1$s"), None);
        assert_eq!(formatter_arg_count("%s %<s"), None);
        assert_eq!(formatter_arg_count("100%"), None);
        assert_eq!(formatter_arg_count("%q"), None);
    }

    #[test]
    fn counts_message_format_arguments() {
        assert_eq!(message_format_arg_count("{0} and {1}"), Some(2));
        assert_eq!(message_format_arg_count("{0} {0}"), Some(1));
        assert_eq!(message_format_arg_count("{1,number,#.#}"), Some(2));
        assert_eq!(
            message_format_arg_count("{0,choice,0#none|1#one {0}}"),
            Some(1)
        );
        assert_eq!(message_format_arg_count("it''s {0}"), None);
        assert_eq!(message_format_arg_count("{x}"), None);
    }

    #[test]
    fn counts_logger_placeholders() {
        assert_eq!(logger_placeholder_count("a {} b {}"), 2);
        assert_eq!(logger_placeholder_count("literal \\{} and {}"), 1);
        assert_eq!(logger_placeholder_count("path \\\\{}"), 1);
        assert_eq!(logger_placeholder_count("{0} {x}"), 0);
    }
}
