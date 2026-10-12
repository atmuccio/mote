//! `java/this-escape-in-constructor` — a constructor registers `this` with another object or
//! starts a thread before the object is fully built.

use super::util::{self, Walk};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["concurrency", "initialization"],
    remediation_minutes: 15,
    description: r#"Detects constructors that let `this` escape before construction is over:
registering it with another object (`source.addListener(this)`, `registry.register(this)`,
`bus.subscribe(this)`) or starting a thread (`thread.start()`).

**Why it matters.** Once another object or thread holds the new instance, it can call it at
any time, including before the rest of the constructor (and the constructors of any
subclass) has run. The callback then sees fields that are still `null` or `0`, and on other
threads the Java memory model does not even guarantee that the fields already assigned are
visible, `final` ones included, because the object was published before its constructor
finished.

**How to fix.** Finish construction first, then publish: register the listener or start the
thread from a factory method or an explicit `start()` / `init()` method called after `new`.

```java
// Noncompliant
class PriceView implements PriceListener {
    PriceView(PriceFeed feed) {
        feed.addListener(this);
        this.format = new DecimalFormat("0.00");   // not yet set when the first update arrives
    }
}

// Compliant
class PriceView implements PriceListener {
    static PriceView create(PriceFeed feed) {
        PriceView view = new PriceView();
        feed.addListener(view);
        return view;
    }
}
```

Off by default: the risk depends on threads and subclasses a single file does not show. Not
reported: the escape as the very last statement of a constructor of a `final` class (nothing
is left to initialize), registrations with objects the instance creates itself (its own child
components, created with `new` in a field initializer or in the constructor), code inside
lambdas or anonymous classes of the constructor, and constructors (or classes) marked
`@SuppressWarnings("this-escape")`, the name javac's lint uses for this warning."#,
    ..RuleMeta::new(
        "java/this-escape-in-constructor",
        "java",
        "this escapes from a constructor",
    )
};

pub struct ThisEscapeInConstructor;

/// Methods that hand the argument to another object for later callbacks.
fn is_registration(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    (lower.starts_with("add")
        && (lower.contains("listener")
            || lower.contains("observer")
            || lower.contains("handler")
            || lower.contains("callback")
            || lower.contains("watcher")))
        || lower.starts_with("register")
        || lower.starts_with("subscribe")
        || lower == "attach"
}

/// Whether the receiver is an object this instance creates itself (a child component): a
/// local or field initialized with `new`, or assigned a `new` object in the constructor. Such
/// an object cannot call back before the instance hands it out.
fn owned_component<'t>(ctx: &Ctx<'t>, recv: Node<'t>, body: Node<'t>) -> bool {
    let recv = syntax::strip_parens(recv);
    let name_node = match kind(recv) {
        "identifier" => recv,
        "field_access" if recv.field(Field::Object).is_some_and(|o| kind(o) == "this") => {
            match recv.field(Field::Field) {
                Some(f) => f,
                None => return false,
            }
        }
        _ => return false,
    };
    let name = ctx.text(name_node);
    let is_new = |v: Node<'t>| kind(syntax::strip_parens(v)) == "object_creation_expression";
    if kind(recv) == "identifier" {
        if let Some(d) = ctx.declaration_of(recv) {
            if d.value.is_some_and(is_new) {
                return true;
            }
            if !matches!(d.kind, VarKind::Local | VarKind::Field) {
                return false;
            }
        }
    }
    // Assigned a new object in the constructor.
    util::visit(body, |n| {
        if util::is_nested_scope(n) {
            return Walk::Skip;
        }
        if kind(n) == "assignment_expression"
            && syntax::operator_str(n) == "="
            && n.field(Field::Right).is_some_and(is_new)
            && n.field(Field::Left).is_some_and(|l| {
                let l = syntax::strip_parens(l);
                match kind(l) {
                    "identifier" => ctx.text(l) == name,
                    "field_access" => {
                        l.field(Field::Object).is_some_and(|o| kind(o) == "this")
                            && l.field(Field::Field).is_some_and(|f| ctx.text(f) == name)
                    }
                    _ => false,
                }
            })
        {
            Walk::Stop
        } else {
            Walk::Continue
        }
    }) || field_initialized_with_new(ctx, recv, name)
}

/// Whether a field of the enclosing class named `name` is declared with a `new` initializer.
fn field_initialized_with_new<'t>(ctx: &Ctx<'t>, at: Node<'t>, name: &str) -> bool {
    let Some(class) = ctx.enclosing_class(at) else {
        return false;
    };
    let Some(body) = syntax::type_body(class) else {
        return false;
    };
    ctx.members(body)
        .filter(|m| kind(*m) == "field_declaration")
        .flat_map(syntax::declarators)
        .any(|d| {
            syntax::name_of(d, ctx.source()) == name
                && d.field(Field::Value)
                    .is_some_and(|v| kind(syntax::strip_parens(v)) == "object_creation_expression")
        })
}

impl<'t> JavaRule<'t> for ThisEscapeInConstructor {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["constructor_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        let text = ctx.text(body);
        if !text.contains("start(") && !passes_this(text) {
            return;
        }
        if util::this_escape_suppressed(ctx, node) {
            return;
        }
        let Some(class) = ctx.enclosing_class(node) else {
            return;
        };
        let final_class = matches!(kind(class), "enum_declaration" | "record_declaration")
            || syntax::has_modifier(class, "final");
        let last = named_children(body).last();
        let mut hits: Vec<(Node<'t>, &'static str)> = Vec::new();
        util::visit(body, |n| {
            if util::is_nested_scope(n) {
                return Walk::Skip;
            }
            if kind(n) != "method_invocation" {
                return Walk::Continue;
            }
            let Some(name) = n.field(Field::Name).map(|x| ctx.text(x)) else {
                return Walk::Continue;
            };
            let Some(obj) = n.field(Field::Object) else {
                return Walk::Continue;
            };
            if kind(obj) == "this" || kind(obj) == "super" {
                return Walk::Continue;
            }
            if name == "start" && util::arg_count(n) == 0 {
                if ctx.is_call_to(n, "java.lang.Thread", "start") {
                    hits.push((n, "starts a thread"));
                }
            } else if is_registration(name)
                && !owned_component(ctx, obj, body)
                && util::args(n).any(|a| kind(syntax::strip_parens(a)) == "this")
            {
                hits.push((n, "registers `this`"));
            }
            Walk::Continue
        });
        for (call, what) in hits {
            if final_class {
                let stmt = ctx
                    .ancestors(call)
                    .take_while(|a| a.id() != body.id())
                    .last()
                    .unwrap_or(call);
                if last.is_some_and(|l| l.id() == stmt.id()) {
                    continue;
                }
            }
            let at = call.field(Field::Name).unwrap_or(call);
            ctx.emit(
                at,
                format!("The constructor {what} before the object is fully built: publish it after construction instead"),
            );
        }
    }
}

/// Whether `this` appears as an argument somewhere in the text (`this` followed by `)` or `,`).
fn passes_this(text: &str) -> bool {
    text.match_indices("this")
        .any(|(i, _)| text[i + 4..].trim_start().starts_with([')', ',']))
}
