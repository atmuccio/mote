//! `java/static-date-format` — non-thread-safe formatters/calendars in static fields.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["concurrency", "multi-threading"],
    remediation_minutes: 10,
    description: r#"Detects `static` fields whose type is `java.text.DateFormat`
(`SimpleDateFormat`), `java.text.NumberFormat` (`DecimalFormat`), `java.text.MessageFormat`, or
`java.util.Calendar`.

**Why it matters.** These classes keep intermediate parsing/formatting state in the instance
and are not thread-safe. A static instance is shared by every thread; concurrent use produces
wrong dates and numbers or random exceptions, typically only under production load.

**How to fix.** Use the immutable, thread-safe `java.time.format.DateTimeFormatter`; or create
the formatter where it is used; or wrap it in a `ThreadLocal`.

```java
// Noncompliant
private static final SimpleDateFormat DAY = new SimpleDateFormat("yyyy-MM-dd");

// Compliant
private static final DateTimeFormatter DAY = DateTimeFormatter.ofPattern("yyyy-MM-dd");
private static final ThreadLocal<DateFormat> LEGACY_DAY =
        ThreadLocal.withInitial(() -> new SimpleDateFormat("yyyy-MM-dd"));
```

Not reported: `private` fields that are only used inside `synchronized` blocks or methods (or
only copied with `clone()`), fields of unknown or other types (e.g. third-party thread-safe
formatters). A `MessageFormat` keeps shared state only in the number, date and choice formats of
its pattern: it is reported when it is created from a literal pattern with a format type
(`{0,number,#.##}`, `{1,date}`); with plain placeholders only (`"Hello {0}"`) formatting creates
what it needs afresh, and a pattern read at run time (a resource bundle) is unknown, so those
are reported only with `report_unknown_message_patterns`."#,
    params: &[ParamSpec {
        name: "report_unknown_message_patterns",
        default: ParamDefault::Bool(false),
        description: "Also report static `MessageFormat` fields whose pattern is not a string literal (read from a resource bundle, built at run time, or set later).",
    }],
    ..RuleMeta::new(
        "java/static-date-format",
        "java",
        "Non-thread-safe formatter in a static field",
    )
};

const UNSAFE_TYPES: &[(&str, &str)] = &[
    ("java.text.DateFormat", "DateFormat"),
    ("java.text.NumberFormat", "NumberFormat"),
    ("java.text.MessageFormat", "MessageFormat"),
    ("java.util.Calendar", "Calendar"),
];

/// Simple names of the unsafe JDK types and their JDK subclasses.
const KNOWN_NAMES: &[&str] = &[
    "DateFormat",
    "SimpleDateFormat",
    "NumberFormat",
    "DecimalFormat",
    "ChoiceFormat",
    "CompactNumberFormat",
    "MessageFormat",
    "Calendar",
    "GregorianCalendar",
];

struct Candidate<'t> {
    decl: Node<'t>,
    name: Node<'t>,
    type_name: &'t str,
    private: bool,
}

#[derive(Default)]
pub struct StaticDateFormat<'t> {
    candidates: Vec<Candidate<'t>>,
}

impl<'t> JavaRule<'t> for StaticDateFormat<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["field_declaration", "constant_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let implicitly_static = kind(node) == "constant_declaration"
            || ctx
                .parent(node)
                .is_some_and(|p| matches!(kind(p), "interface_body" | "annotation_type_body"));
        if !implicitly_static && !syntax::has_modifier(node, "static") {
            return;
        }
        let Some(type_node) = node.field(Field::Type) else {
            return;
        };
        // Cheap reject: arrays, primitives, generic types (ThreadLocal<…>, …).
        if !matches!(
            kind(type_node),
            "type_identifier" | "scoped_type_identifier"
        ) {
            return;
        }
        // Only the JDK classes and their JDK subclasses can be proven unsafe.
        let simple = ctx.text(type_node).rsplit('.').next().unwrap_or("");
        if !KNOWN_NAMES.contains(&simple) {
            return;
        }
        let Some(t) = ctx.resolve_type(type_node) else {
            return;
        };
        if !UNSAFE_TYPES
            .iter()
            .any(|(fqn, _)| ctx.is_subtype_of(&t, fqn))
        {
            return;
        }
        let private = !implicitly_static && syntax::has_modifier(node, "private");
        let message_format = t.is_class("java.text.MessageFormat");
        for d in syntax::declarators(node) {
            if message_format {
                let report = match message_pattern(ctx, d) {
                    Pattern::WithFormatTypes => true,
                    Pattern::Plain => false,
                    Pattern::Unknown => ctx.param_bool("report_unknown_message_patterns"),
                };
                if !report {
                    continue;
                }
            }
            if let Some(name) = util::name_field(d) {
                self.candidates.push(Candidate {
                    decl: node,
                    name,
                    type_name: ctx.text(type_node),
                    private,
                });
            }
        }
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        for c in &self.candidates {
            if c.private && only_used_safely(c, ctx) {
                continue;
            }
            let name = ctx.text(c.name);
            ctx.emit(
                c.name,
                format!(
                    "`{}` is not thread-safe; don't share it through the static field `{name}`",
                    c.type_name
                ),
            );
        }
    }
}

/// Whether every use of the private field happens under a lock (or is a `clone()` copy).
/// Scans the outermost type declaration containing the field (private members are visible
/// there). Only runs for the rare candidates, so the bounded subtree walk is affordable.
fn only_used_safely<'t>(c: &Candidate<'t>, ctx: &Ctx<'t>) -> bool {
    let Some(top) = ctx
        .ancestors(c.decl)
        .filter(|a| syntax::is_type_declaration(*a))
        .last()
    else {
        return false;
    };
    let name = ctx.text(c.name);
    let mut uses = 0usize;
    // (node, inside a lock, parent, grandparent); an explicit stack keeps deep trees off the
    // call stack.
    type Entry<'t> = (Node<'t>, bool, Option<Node<'t>>, Option<Node<'t>>);
    let mut stack: Vec<Entry<'t>> = vec![(top, false, None, None)];
    while let Some((n, locked, parent, grand)) = stack.pop() {
        let k = kind(n);
        if k == "identifier" {
            if n.id() != c.name.id() && ctx.text(n) == name {
                uses += 1;
                if !locked && !is_clone_receiver(n, parent, grand, ctx) {
                    return false;
                }
            }
            continue;
        }
        let child_locked = match k {
            "method_declaration" => syntax::has_modifier(n, "synchronized"),
            "constructor_declaration" | "lambda_expression" | "class_body" => false,
            "synchronized_statement" => true,
            _ => locked,
        };
        // (Using the field as the lock object only reads the reference: that is safe too.)
        let mut cur = n.walk();
        for ch in n.named_children(&mut cur) {
            stack.push((ch, child_locked, Some(n), parent));
        }
    }
    uses > 0
}

/// What the pattern of a `MessageFormat` field tells about shared state.
enum Pattern {
    /// A literal pattern naming a format type (`{0,number}`): the field shares that format.
    WithFormatTypes,
    /// A literal pattern with plain placeholders only: nothing shared is used by `format`.
    Plain,
    /// Not a literal (or quoting makes it unclear).
    Unknown,
}

/// The pattern of `new MessageFormat("…")` initializing `declarator`.
fn message_pattern<'t>(ctx: &Ctx<'t>, declarator: Node<'t>) -> Pattern {
    let Some(value) = declarator.field(Field::Value).map(syntax::strip_parens) else {
        return Pattern::Unknown;
    };
    if kind(value) != "object_creation_expression" {
        return Pattern::Unknown;
    }
    let Some(pattern) = value
        .field(Field::Arguments)
        .and_then(|a| syntax::named_children(a).next())
    else {
        return Pattern::Unknown;
    };
    let Some(text) = syntax::string_literal_content(pattern, ctx.source()) else {
        return Pattern::Unknown;
    };
    // Quoted text (`'{'`) is rare in patterns; a quote makes the placeholders unclear.
    if text.contains('\'') {
        return Pattern::Unknown;
    }
    let mut depth = 0u32;
    for c in text.chars() {
        match c {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            ',' if depth > 0 => return Pattern::WithFormatTypes,
            _ => {}
        }
    }
    Pattern::Plain
}

/// `NAME.clone()` or `this.NAME.clone()`: a private copy, safe to use afterwards.
fn is_clone_receiver<'t>(
    ident: Node<'t>,
    parent: Option<Node<'t>>,
    grand: Option<Node<'t>>,
    ctx: &Ctx<'t>,
) -> bool {
    let (recv, call) = match parent {
        Some(p) if kind(p) == "field_access" => (p, grand),
        Some(p) => (ident, Some(p)),
        None => return false,
    };
    call.is_some_and(|call| {
        kind(call) == "method_invocation"
            && call
                .field(Field::Object)
                .is_some_and(|o| o.id() == recv.id())
            && call
                .field(Field::Name)
                .is_some_and(|m| ctx.text(m) == "clone")
    })
}
