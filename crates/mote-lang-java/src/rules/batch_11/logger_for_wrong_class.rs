//! `java/logger-for-wrong-class` — a logger field created for another class than the one that
//! declares it.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["logging", "copy-paste"],
    remediation_minutes: 1,
    description: r#"Detects a logger field initialized for another class than the one declaring it:
`private static final Logger log = LoggerFactory.getLogger(OrderService.class);` inside
`class InvoiceService` (SLF4J, Log4j 2, Log4j 1, Commons Logging, JBoss Logging and
`java.util.logging` factories, with a class literal or its `getName()`).

**Why it matters.** The class passed to the factory names the log category: it is what the log
lines show as their origin and what logging configuration uses to turn levels up or down. A
logger copied from another class together with the code around it files every message under
the wrong class, so the messages are searched for in the wrong place and level settings meant
for one class silently apply to the other.

**How to fix.** Pass the declaring class.

```java
// Noncompliant
class InvoiceService {
    private static final Logger log = LoggerFactory.getLogger(OrderService.class);
}

// Compliant
class InvoiceService {
    private static final Logger log = LoggerFactory.getLogger(InvoiceService.class);
}
```

Not reported: nested classes using an enclosing class (a common way to share one category),
classes using one of their declared supertypes (a subclass logging under its base class's
category is a deliberate choice), loggers created from a string or from `getClass()`, loggers
created outside field initializers, and test code."#,
    ..RuleMeta::new(
        "java/logger-for-wrong-class",
        "java",
        "Logger created for another class",
    )
};

pub struct LoggerForWrongClass;

/// Logger factory classes.
const FACTORIES: &[&str] = &[
    "org.slf4j.LoggerFactory",
    "org.apache.logging.log4j.LogManager",
    "org.apache.log4j.Logger",
    "org.apache.log4j.LogManager",
    "org.apache.commons.logging.LogFactory",
    "java.util.logging.Logger",
    "org.jboss.logging.Logger",
];

/// For `X.class` or `X.class.getName()` (and `getSimpleName`, `getCanonicalName`): the type
/// node `X`.
fn named_class<'t>(ctx: &Ctx<'t>, arg: Node<'t>) -> Option<Node<'t>> {
    let arg = syntax::strip_parens(arg);
    match kind(arg) {
        "class_literal" => syntax::first_named_child(arg),
        "method_invocation" => {
            if !matches!(ctx.call_name(arg), Some("getName" | "getCanonicalName"))
                || syntax::argument_count(arg) != 0
            {
                return None;
            }
            let obj = syntax::strip_parens(arg.field(Field::Object)?);
            (kind(obj) == "class_literal")
                .then(|| syntax::first_named_child(obj))
                .flatten()
        }
        _ => None,
    }
}

/// Simple names of the supertypes a type declaration lists.
fn supertype_names<'t>(ctx: &Ctx<'t>, decl: Node<'t>, out: &mut Vec<&'t str>) {
    let mut lists: Vec<Node<'t>> = Vec::new();
    if let Some(sc) = decl.field(Field::Superclass) {
        lists.push(sc);
    }
    lists.extend(decl.field(Field::Interfaces));
    lists.extend(syntax::child_of_kind(decl, "extends_interfaces"));
    for l in lists {
        util::visit(l, |n| {
            if matches!(kind(n), "type_identifier") {
                out.push(ctx.text(n));
            }
            util::Walk::Continue
        });
    }
}

/// Whether a comment of its own (not trailing code) ends on the line just above `decl`.
fn has_adjacent_comment(ctx: &Ctx<'_>, decl: Node<'_>) -> bool {
    let first_line = decl.start_position().row as u32 + 1;
    ctx.comments()
        .iter()
        .rev()
        .take(8)
        .any(|c| !c.trailing && c.end_line + 1 == first_line)
}

impl<'t> JavaRule<'t> for LoggerForWrongClass {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["getLogger", "getLog"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !matches!(ctx.call_name(node), Some("getLogger" | "getLog")) || ctx.is_test() {
            return;
        }
        let Some(arg) = util::arg(node, 0) else {
            return;
        };
        let Some(class_node) = named_class(ctx, arg) else {
            return;
        };
        // A field initializer: call ← variable_declarator ← field_declaration ← class body.
        let Some(declarator) = ctx.parent(node) else {
            return;
        };
        if kind(declarator) != "variable_declarator" {
            return;
        }
        let Some(field) = ctx.parent(declarator) else {
            return;
        };
        if kind(field) != "field_declaration" {
            return;
        }
        let Some(recv) = node.field(Field::Object) else {
            return;
        };
        let Some(factory) = ctx.type_name_of(recv) else {
            return;
        };
        if !FACTORIES.contains(&factory.as_str()) {
            return;
        }
        let used = util::last_segment(ctx.text(util::type_base(class_node)));
        // The declaring class and the classes around it, with their supertypes.
        let mut accepted: Vec<&'t str> = Vec::new();
        let mut enclosing: Vec<&'t str> = Vec::new();
        for a in ctx.ancestors(node) {
            if kind(a) == "object_creation_expression" || kind(a) == "enum_constant" {
                // Anonymous class: its logger belongs to the enclosing class.
                continue;
            }
            if !syntax::is_type_declaration(a) {
                continue;
            }
            let name = syntax::name_of(a, ctx.source());
            enclosing.push(name);
            accepted.push(name);
            supertype_names(ctx, a, &mut accepted);
        }
        let Some(&declaring) = enclosing.first() else {
            return;
        };
        if accepted.contains(&used) {
            return;
        }
        // A category of a closely related class: `DefaultFoo` or `FooImpl` inside `Foo`.
        if enclosing
            .iter()
            .any(|e| used.starts_with(e) || used.ends_with(e))
        {
            return;
        }
        // An explained choice: a comment right before the field.
        if has_adjacent_comment(ctx, field) {
            return;
        }
        ctx.emit(
            class_node,
            format!(
                "Create this logger for `{declaring}`, the class declaring it, not for `{used}`"
            ),
        );
    }
}
