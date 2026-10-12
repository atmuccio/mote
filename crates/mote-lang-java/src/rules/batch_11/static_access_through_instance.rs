//! `java/static-access-through-instance` — a static member reached through an instance
//! expression or through a subclass name.

use super::util;
use crate::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["confusing", "static"],
    remediation_minutes: 2,
    description: r#"Detects a static method or field reached through an expression of an instance
(`worker.sleep(100)`, `order.COUNTER`) or through the name of a subclass that inherits it
(`MyThread.currentThread()`), when the member is declared in this file or is one of the JDK's
well-known static methods (`Thread.sleep`, `Thread.currentThread`, `Thread.interrupted`,
`String.valueOf`, `String.format`, `Pattern.compile`, the wrapper classes' `parseXxx` and
`valueOf`, …).

**Why it matters.** The call reads as if it acted on that object, but the object is not used
at all — only its declared type, and the expression is not even required to be non-null.
`worker.sleep(100)` pauses the *current* thread, not `worker`; `thread.interrupted()` clears
the flag of the current thread instead of testing `thread`'s. Through a subclass name, the
reader looks for the member in the wrong class.

**How to fix.** Qualify the member with the class that declares it.

```java
// Noncompliant
worker.sleep(100);
if (thread.interrupted()) { … }

// Compliant
Thread.sleep(100);
if (thread.isInterrupted()) { … }
```

Not reported: `this.` and `super.` qualifiers, members of types declared in other files (their
modifiers are unknown), overloads where an instance method of the same name exists with the
same number of parameters, and receivers whose type the resolver does not know."#,
    ..RuleMeta::new(
        "java/static-access-through-instance",
        "java",
        "Static member accessed through an instance",
    )
};

#[derive(Default)]
pub struct StaticAccessThroughInstance<'t> {
    /// Names of the static methods and fields declared in the types of this file, built on
    /// first use.
    statics: Option<FxHashSet<&'t str>>,
    /// Simple names of the types declared in this file (built with `statics`).
    types: FxHashSet<&'t str>,
    /// Per-file memo of the type declarations found by fully-qualified name.
    found: RefCell<FxHashMap<String, Option<Node<'t>>>>,
    /// Per-file memo of [`file_static_member`] at depth 0 → declaring type.
    declaring: RefCell<FxHashMap<MemberKey<'t>, Option<&'t str>>>,
    /// The methods and fields of each type body asked about, by name (see [`MembersByName`]).
    by_name: MembersByName<'t>,
}

/// Type body id → member name → the methods and field declarations of that name, in order:
/// [`file_static_member`] looks a name up instead of going through every member (a class of
/// half a million members took a third of a second per name).
type MembersByName<'t> = RefCell<FxHashMap<usize, Rc<FxHashMap<&'t str, Vec<Node<'t>>>>>>;

/// The methods and field declarations of `body` named `name` (see [`MembersByName`]).
fn members_named<'t>(
    ctx: &Ctx<'t>,
    cache: &MembersByName<'t>,
    body: Node<'t>,
    name: &str,
) -> Vec<Node<'t>> {
    let cached = cache.borrow().get(&body.id()).cloned();
    let index = match cached {
        Some(index) => index,
        None => {
            let src = ctx.source();
            let mut index: FxHashMap<&'t str, Vec<Node<'t>>> = FxHashMap::default();
            for m in ctx.members(body) {
                match kind(m) {
                    "method_declaration" => {
                        index.entry(syntax::name_of(m, src)).or_default().push(m);
                    }
                    "field_declaration" => {
                        for d in syntax::declarators(m) {
                            let list = index.entry(syntax::name_of(d, src)).or_default();
                            if list.last().is_none_or(|l| l.id() != m.id()) {
                                list.push(m);
                            }
                        }
                    }
                    _ => {}
                }
            }
            let index = Rc::new(index);
            cache.borrow_mut().insert(body.id(), index.clone());
            index
        }
    };
    index.get(name).cloned().unwrap_or_default()
}

/// (declaration id, member name, method arity).
type MemberKey<'t> = (usize, &'t str, Option<usize>);

/// Static member names of the type declarations of `scope` (recursively, bounded).
fn collect_statics<'t>(
    ctx: &Ctx<'t>,
    scope: Node<'t>,
    out: &mut FxHashSet<&'t str>,
    types: &mut FxHashSet<&'t str>,
    depth: u32,
) {
    if depth > 6 {
        return;
    }
    if kind(scope) == "program" {
        for m in named_children(scope) {
            collect_member(ctx, m, out, types, depth);
        }
    } else if let Some(body) = syntax::type_body(scope) {
        for m in ctx.members(body) {
            collect_member(ctx, m, out, types, depth);
        }
    }
}

/// One member of [`collect_statics`].
fn collect_member<'t>(
    ctx: &Ctx<'t>,
    m: Node<'t>,
    out: &mut FxHashSet<&'t str>,
    types: &mut FxHashSet<&'t str>,
    depth: u32,
) {
    let src = ctx.source();
    match kind(m) {
        "method_declaration" if is_static(ctx, m) => {
            out.insert(syntax::name_of(m, src));
        }
        "field_declaration" if is_static(ctx, m) => {
            for d in syntax::declarators(m) {
                out.insert(syntax::name_of(d, src));
            }
        }
        _ if syntax::is_type_declaration(m) => {
            types.insert(syntax::name_of(m, src));
            collect_statics(ctx, m, out, types, depth + 1)
        }
        _ => {}
    }
}

/// `syntax::has_modifier(decl, "static")`, skipping the token scan when the modifiers' text
/// does not even contain the word.
fn is_static<'t>(ctx: &Ctx<'t>, decl: Node<'t>) -> bool {
    syntax::modifiers(decl).is_some_and(|m| ctx.text(m).contains("static"))
        && syntax::has_modifier(decl, "static")
}

/// JDK static methods: owner, name, and the number of arguments for which the method is
/// static (`None`: every arity).
const JDK_STATICS: &[(&str, &str, Option<usize>)] = &[
    ("java.lang.Thread", "sleep", None),
    ("java.lang.Thread", "currentThread", None),
    ("java.lang.Thread", "interrupted", None),
    ("java.lang.Thread", "yield", None),
    ("java.lang.Thread", "holdsLock", None),
    ("java.lang.Thread", "activeCount", None),
    ("java.lang.Thread", "dumpStack", None),
    ("java.lang.Thread", "enumerate", None),
    ("java.lang.Thread", "onSpinWait", None),
    ("java.lang.Thread", "getAllStackTraces", None),
    (
        "java.lang.Thread",
        "setDefaultUncaughtExceptionHandler",
        None,
    ),
    (
        "java.lang.Thread",
        "getDefaultUncaughtExceptionHandler",
        None,
    ),
    ("java.lang.String", "valueOf", None),
    ("java.lang.String", "format", None),
    ("java.lang.String", "join", None),
    ("java.lang.String", "copyValueOf", None),
    ("java.util.regex.Pattern", "compile", None),
    ("java.util.regex.Pattern", "quote", None),
    ("java.util.regex.Pattern", "matches", Some(2)),
    ("java.lang.Integer", "parseInt", None),
    ("java.lang.Integer", "valueOf", None),
    ("java.lang.Integer", "toString", Some(1)),
    ("java.lang.Long", "parseLong", None),
    ("java.lang.Long", "valueOf", None),
    ("java.lang.Long", "toString", Some(1)),
    ("java.lang.Double", "parseDouble", None),
    ("java.lang.Double", "valueOf", None),
    ("java.lang.Double", "isNaN", Some(1)),
    ("java.lang.Float", "parseFloat", None),
    ("java.lang.Float", "valueOf", None),
    ("java.lang.Boolean", "parseBoolean", None),
    ("java.lang.Boolean", "valueOf", None),
];

/// The method names of [`JDK_STATICS`], for a hash lookup on every call.
static JDK_STATIC_NAMES: std::sync::LazyLock<FxHashSet<&'static str>> =
    std::sync::LazyLock::new(|| JDK_STATICS.iter().map(|(_, m, _)| *m).collect());

/// Whether `name` is one of the JDK static method names of [`JDK_STATICS`].
fn is_jdk_static_name(name: &str) -> bool {
    JDK_STATIC_NAMES.contains(name)
}

/// The static JDK method `name` with `n` arguments on type `t`, as its declaring class.
fn jdk_static<'t>(ctx: &Ctx<'t>, t: &JType, name: &str, n: usize) -> Option<&'static str> {
    JDK_STATICS
        .iter()
        .find(|(owner, m, arity)| {
            *m == name
                && arity.is_none_or(|a| a == n)
                && (t.is_class(owner)
                    || (*owner == "java.lang.Thread" && ctx.is_subtype_of(t, owner)))
        })
        .map(|(owner, _, _)| *owner)
}

/// Whether the type declared in this file as `decl` (or a class of this file it extends)
/// declares `name` only as a static method (with `n` parameters) / static field. Returns the
/// declaring type's simple name.
fn file_static_member<'t>(
    ctx: &Ctx<'t>,
    find: &dyn Fn(&str) -> Option<Node<'t>>,
    by_name: &MembersByName<'t>,
    decl: Node<'t>,
    name: &str,
    method_arity: Option<usize>,
    depth: u32,
) -> Option<&'t str> {
    if depth > 6 {
        return None;
    }
    let src = ctx.source();
    let body = syntax::type_body(decl)?;
    let mut found_static = false;
    for m in members_named(ctx, by_name, body, name) {
        match (kind(m), method_arity) {
            ("method_declaration", Some(n)) => {
                if syntax::name_of(m, src) != name {
                    continue;
                }
                let params = syntax::parameters(m).count();
                let varargs = syntax::parameters(m).any(|p| kind(p) == "spread_parameter");
                if params != n && !varargs {
                    continue;
                }
                if !syntax::has_modifier(m, "static") {
                    return None;
                }
                found_static = true;
            }
            ("field_declaration", None)
                if syntax::declarators(m).any(|d| syntax::name_of(d, src) == name) =>
            {
                if !syntax::has_modifier(m, "static") {
                    return None;
                }
                found_static = true;
            }
            _ => {}
        }
    }
    if found_static {
        return Some(syntax::name_of(decl, src));
    }
    // Inherited from a superclass of this file.
    let sc = decl
        .field(Field::Superclass)
        .and_then(syntax::first_named_child)?;
    let fqn = util::type_fqn(ctx, sc)?;
    let parent = find(&fqn)?;
    file_static_member(ctx, find, by_name, parent, name, method_arity, depth + 1)
}

/// Whether `expr` is an instance expression (not a type name, not `this` / `super`).
fn is_instance<'t>(ctx: &Ctx<'t>, expr: Node<'t>) -> bool {
    let e = syntax::strip_parens(expr);
    match kind(e) {
        "this" | "super" => false,
        "identifier" | "field_access" => {
            // `Outer.this.x` style and type names are excluded.
            if kind(e) == "field_access" && e.field(Field::Field).is_some_and(|f| kind(f) == "this")
            {
                return false;
            }
            ctx.type_name_of(e).is_none()
        }
        "method_invocation" | "array_access" | "object_creation_expression" => true,
        _ => false,
    }
}

impl<'t> JavaRule<'t> for StaticAccessThroughInstance<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation", "field_access"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let is_call = kind(node) == "method_invocation";
        let name = if is_call {
            let Some(name) = ctx.call_name(node) else {
                return;
            };
            name
        } else {
            let Some(f) = node.field(Field::Field) else {
                return;
            };
            if kind(f) != "identifier" {
                return;
            }
            ctx.text(f)
        };
        // Cheap filter first: a static member of this file, or a JDK static method name.
        if self.statics.is_none() {
            let mut s = FxHashSet::default();
            let mut t = FxHashSet::default();
            collect_statics(ctx, ctx.root(), &mut s, &mut t, 0);
            self.statics = Some(s);
            self.types = t;
        }
        let Some(statics) = self.statics.as_ref() else {
            return;
        };
        let types = &self.types;
        // Only types declared in this file are looked up in its tree (`types` holds the
        // names of all of them, so the search could find nothing else); answers are memoized.
        let found = &self.found;
        let find = |fqn: &str| -> Option<Node<'t>> {
            if !types.contains(util::last_segment(fqn)) {
                return None;
            }
            if let Some(d) = found.borrow().get(fqn) {
                return *d;
            }
            let d = util::find_type_decl(ctx, ctx.root(), fqn, 0);
            found.borrow_mut().insert(fqn.to_string(), d);
            d
        };
        let declaring_memo = &self.declaring;
        let member_of = |decl: Node<'t>, arity: Option<usize>| -> Option<&'t str> {
            let key = (decl.id(), name, arity);
            if let Some(d) = declaring_memo.borrow().get(&key) {
                return *d;
            }
            let d = file_static_member(ctx, &find, &self.by_name, decl, name, arity, 0);
            declaring_memo.borrow_mut().insert(key, d);
            d
        };
        let file_static = statics.contains(name);
        if !(file_static || (is_call && is_jdk_static_name(name))) {
            return;
        }
        let Some(obj) = node.field(Field::Object) else {
            return;
        };
        let arity = is_call.then(|| util::arg_count(node));
        if !file_static
            && !JDK_STATICS
                .iter()
                .any(|(_, m, a)| *m == name && a.is_none_or(|a| Some(a) == arity))
        {
            return;
        }
        let obj_kind = kind(syntax::strip_parens(obj));
        if matches!(obj_kind, "this" | "super") {
            return;
        }
        // A capitalized name that no variable of the file declares is a type name; only types
        // of this file are of interest there (a quick exit before any resolver work).
        if obj_kind == "identifier" {
            let text = ctx.text(obj);
            let declared = ctx.declared_types().type_of_name(text);
            if text.starts_with(|c: char| c.is_ascii_uppercase())
                && declared.is_none()
                && ctx.declared_types().types_of(text).next().is_none()
                && !types.contains(text)
            {
                return;
            }
            // A variable whose declared type is neither a type of this file nor one of the
            // JDK classes of the table cannot reach one of the statics looked for.
            if let Some(ty) = declared {
                let simple = util::last_segment(ctx.text(util::type_base(ty)));
                if !types.contains(simple)
                    && !JDK_STATICS
                        .iter()
                        .any(|(owner, _, _)| util::last_segment(owner) == simple)
                {
                    return;
                }
            }
        }
        // Through a type name: only a subclass name is suspicious.
        if matches!(
            obj_kind,
            "identifier" | "field_access" | "scoped_identifier"
        ) {
            if let Some(type_fqn) = ctx.type_name_of(obj) {
                let Some(decl) = find(&type_fqn) else {
                    // A JDK static through a subclass of Thread declared elsewhere: unknown.
                    return;
                };
                let simple = syntax::name_of(decl, ctx.source());
                let declaring = match arity {
                    Some(n) => member_of(decl, Some(n)).or_else(|| {
                        // `MyThread.sleep()` with `MyThread extends Thread`.
                        let t = ctx.class_type(decl)?;
                        if !ctx.is_subtype_of(&t, "java.lang.Thread")
                            || find("java.lang.Thread").is_some()
                        {
                            return None;
                        }
                        let declares_own = syntax::type_body(decl).is_some_and(|b| {
                            ctx.members(b).any(|m| {
                                kind(m) == "method_declaration"
                                    && syntax::name_of(m, ctx.source()) == name
                            })
                        });
                        if declares_own {
                            return None;
                        }
                        jdk_static(ctx, &JType::class("java.lang.Thread"), name, n)
                            .map(util::last_segment)
                    }),
                    None => member_of(decl, None),
                };
                let Some(declaring) = declaring else {
                    return;
                };
                if declaring == simple {
                    return;
                }
                ctx.emit(
                    obj,
                    format!(
                        "Access the static member `{name}` through `{declaring}`, the class \
                         declaring it, not through the subclass `{simple}`"
                    ),
                );
                return;
            }
        }
        if !is_instance(ctx, obj) {
            return;
        }
        let Some(t) = ctx.type_of(syntax::strip_parens(obj)) else {
            return;
        };
        let declaring = match arity {
            Some(n) => jdk_static(ctx, &t, name, n)
                .map(util::last_segment)
                .or_else(|| {
                    let fqn = t.fqn()?;
                    let decl = find(fqn)?;
                    member_of(decl, Some(n))
                }),
            None => t.fqn().and_then(|fqn| {
                let decl = find(fqn)?;
                member_of(decl, None)
            }),
        };
        let Some(declaring) = declaring else {
            return;
        };
        let what = if arity.is_some() { "method" } else { "field" };
        let at = if kind(node) == "method_invocation" {
            node.field(Field::Name).unwrap_or(node)
        } else {
            node
        };
        ctx.emit(
            at,
            format!(
                "Access the static {what} `{name}` through its class `{declaring}`, not through \
                 an instance"
            ),
        );
    }
}
