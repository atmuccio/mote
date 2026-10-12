//! `java/static-field-set-in-constructor` — a constructor assigns a static field of its own
//! class.

use super::util::{self, Walk};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["suspicious", "design"],
    remediation_minutes: 10,
    description: r#"Detects constructors that assign (`=`) a non-final `static` field of their own
class.

**Why it matters.** A static field is shared by all instances, but a constructor runs once per
instance: every new object silently overwrites the value that the others rely on. Code that
reads the field through one instance then sees the state of whichever instance was created
last, and concurrent construction makes the result depend on timing. It usually means the
field was meant to be an instance field (a missing or stray `static`), or that a singleton
is registered from its constructor, which leaks `this` before it is fully built.

**How to fix.** Remove `static` if the value belongs to each instance. If one shared value is
intended, set it once in a static initializer or a static factory method.

```java
// Noncompliant
class Connection {
    private static String url;
    Connection(String url) {
        Connection.url = url;       // the last connection created wins
    }
}

// Compliant
class Connection {
    private final String url;
    Connection(String url) {
        this.url = url;
    }
}
```

Not reported: compound updates such as `count++` or `total += n` (instance counters are
deliberate shared state), writes inside a `synchronized` block, writes explained by a comment
on the same line or the line above (a documented, deliberate choice), classes whose
constructors are all private (the class controls how often they run, as in a singleton), and
test code (test doubles that reset a static flag for each test)."#,
    ..RuleMeta::new(
        "java/static-field-set-in-constructor",
        "java",
        "Static field assigned in a constructor",
    )
};

/// Per-class facts, computed once for all constructors of a class.
#[derive(Default)]
pub struct StaticFieldSetInConstructor<'t> {
    /// Class id and the names of its non-final static fields (empty when the class cannot be
    /// instantiated from outside).
    cache: Option<(usize, Vec<&'t str>)>,
}

/// The non-final static fields of `class`; none when all its constructors are private (a
/// singleton: the class decides how often the constructor runs, usually once).
fn class_statics<'t>(ctx: &Ctx<'t>, class: Node<'t>) -> Vec<&'t str> {
    let src = ctx.source();
    let Some(class_body) = syntax::type_body(class) else {
        return Vec::new();
    };
    let mut statics = Vec::new();
    let mut ctors = 0;
    let mut private_ctors = 0;
    for m in ctx.members(class_body) {
        match kind(m) {
            "field_declaration"
                if syntax::has_modifier(m, "static") && !syntax::has_modifier(m, "final") =>
            {
                statics.extend(syntax::declarators(m).map(|d| syntax::name_of(d, src)));
            }
            "constructor_declaration" => {
                ctors += 1;
                if syntax::has_modifier(m, "private") {
                    private_ctors += 1;
                }
            }
            _ => {}
        }
    }
    if ctors == private_ctors {
        statics.clear();
    }
    statics
}

/// The field name an assignment target names: `f`, `this.f`, or `Owner.f`.
fn target_name<'t>(ctx: &Ctx<'t>, left: Node<'t>) -> Option<(Node<'t>, &'t str)> {
    let left = syntax::strip_parens(left);
    match kind(left) {
        "identifier" => Some((left, ctx.text(left))),
        "field_access" => {
            let f = left.field(Field::Field)?;
            (kind(f) == "identifier").then(|| (f, ctx.text(f)))
        }
        _ => None,
    }
}

/// Whether the write is explained by a comment: one at the end of its line, or one on the line
/// right above it.
fn documented(ctx: &Ctx<'_>, assign: Node<'_>) -> bool {
    let src = ctx.source();
    let (start, end) = (assign.start_byte(), assign.end_byte());
    let rest = src.get(end..).unwrap_or("");
    // Tool directives (fixture markers, suppressions) are not explanations; `mote-` must start a
    // word (`remote-host` is prose).
    let directive = |t: &str| {
        t.contains("expect:")
            || t.contains("expect-next-line:")
            || t.match_indices("mote-").any(|(at, _)| {
                !t[..at]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '-')
            })
    };
    let rest_of_line = rest.split('\n').next().unwrap_or("");
    if (rest_of_line.contains("//") || rest_of_line.contains("/*")) && !directive(rest_of_line) {
        return true;
    }
    let before = src.get(..start).unwrap_or("");
    let Some(line_start) = before.rfind('\n') else {
        return false;
    };
    let prev = before[..line_start]
        .rsplit('\n')
        .next()
        .unwrap_or("")
        .trim_start();
    (prev.starts_with("//") || prev.starts_with("/*") || prev.starts_with('*')) && !directive(prev)
}

impl<'t> JavaRule<'t> for StaticFieldSetInConstructor<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["constructor_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        // Test doubles reset static flags from their constructors on purpose.
        if ctx.is_test() {
            return;
        }
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        if !ctx.text(body).contains('=') {
            return;
        }
        let Some(class) = ctx.enclosing_class(node) else {
            return;
        };
        if self.cache.as_ref().is_none_or(|c| c.0 != class.id()) {
            self.cache = Some((class.id(), class_statics(ctx, class)));
        }
        let Some((_, statics)) = self.cache.as_ref() else {
            return;
        };
        if statics.is_empty() {
            return;
        }
        let mut hits = Vec::new();
        util::visit(body, |n| match kind(n) {
            "synchronized_statement" => Walk::Skip,
            _ if util::is_nested_scope(n) => Walk::Skip,
            "assignment_expression" => {
                if syntax::operator_str(n) == "=" {
                    if let Some((at, name)) = n.field(Field::Left).and_then(|l| target_name(ctx, l))
                    {
                        if statics.contains(&name) {
                            hits.push((n, at, name));
                        }
                    }
                }
                Walk::Continue
            }
            _ => Walk::Continue,
        });
        for (assign, at, name) in hits {
            if documented(ctx, assign) {
                continue;
            }
            // Confirm that the target is the static field (not a local or parameter).
            let Some(left) = assign.field(Field::Left).map(syntax::strip_parens) else {
                continue;
            };
            let is_field = match kind(left) {
                "identifier" => ctx.declaration_of(left).is_some_and(|d| {
                    d.kind == VarKind::Field && syntax::has_modifier(d.statement, "static")
                }),
                "field_access" => {
                    let Some(obj) = left.field(Field::Object) else {
                        continue;
                    };
                    kind(obj) == "this"
                        || (kind(obj) == "identifier"
                            && ctx.text(obj) == syntax::name_of(class, ctx.source()))
                }
                _ => false,
            };
            if !is_field {
                continue;
            }
            ctx.emit(
                at,
                format!(
                    "This constructor overwrites the static field `{name}` shared by all instances"
                ),
            );
        }
    }
}
