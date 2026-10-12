//! `java/unsynchronized-accessor` — one of a getter/setter pair is `synchronized`, the other
//! is not.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["concurrency", "multi-threading"],
    remediation_minutes: 5,
    description: r#"Detects a getter and a setter of the same field where one is declared
`synchronized` and the other is not: `synchronized void setLimit(int v) { limit = v; }` next
to `int getLimit() { return limit; }`.

**Why it matters.** Making one accessor `synchronized` shows that the field is shared between
threads. But the lock only protects code that takes it: without it, the other accessor may
read a stale value that never reflects the last write (the Java memory model gives no
visibility guarantee without a common lock or `volatile`), and a `long` or `double` may even
be read half-written. The half-applied synchronization gives a false sense of safety.

**How to fix.** Make both accessors `synchronized`, or drop the locks and declare the field
`volatile` when a single read or write is all that is needed.

```java
// Noncompliant
public synchronized void setLimit(int limit) { this.limit = limit; }
public int getLimit() { return limit; }

// Compliant
public synchronized void setLimit(int limit) { this.limit = limit; }
public synchronized int getLimit() { return limit; }
```

Only one-statement accessors named `getX`/`isX` and `setX` of a non-`volatile`, non-`final`
field declared in the same class are considered, both static or both instance methods.
Classes with Lombok annotations are skipped."#,
    ..RuleMeta::new(
        "java/unsynchronized-accessor",
        "java",
        "Getter/setter pair with inconsistent synchronization",
    )
};

pub struct UnsynchronizedAccessor;

/// The field an expression names: `f` or `this.f`.
fn field_ref<'t>(ctx: &Ctx<'t>, e: Node<'t>) -> Option<&'t str> {
    let e = syntax::strip_parens(e);
    match kind(e) {
        "identifier" => Some(ctx.text(e)),
        "field_access" => {
            let (o, f) = (e.field(Field::Object)?, e.field(Field::Field)?);
            (kind(o) == "this" && kind(f) == "identifier").then(|| ctx.text(f))
        }
        _ => None,
    }
}

/// The only statement of a method body.
fn single_statement(m: Node<'_>) -> Option<Node<'_>> {
    let body = m.field(Field::Body)?;
    let mut it = named_children(body);
    let first = it.next()?;
    it.next().is_none().then_some(first)
}

/// `getX() { return f; }` → (`X`, `f`).
fn getter<'t>(ctx: &Ctx<'t>, m: Node<'t>) -> Option<(&'t str, &'t str)> {
    let name = syntax::name_of(m, ctx.source());
    let prop = name
        .strip_prefix("get")
        .or_else(|| name.strip_prefix("is"))
        .filter(|p| p.starts_with(|c: char| c.is_ascii_uppercase()))?;
    if syntax::parameters(m).next().is_some() {
        return None;
    }
    let s = single_statement(m)?;
    if kind(s) != "return_statement" {
        return None;
    }
    Some((prop, field_ref(ctx, syntax::first_named_child(s)?)?))
}

/// `setX(v) { f = v; }` → (`X`, `f`).
fn setter<'t>(ctx: &Ctx<'t>, m: Node<'t>) -> Option<(&'t str, &'t str)> {
    let name = syntax::name_of(m, ctx.source());
    let prop = name
        .strip_prefix("set")
        .filter(|p| p.starts_with(|c: char| c.is_ascii_uppercase()))?;
    let mut params = syntax::parameters(m);
    let param = params.next()?;
    if params.next().is_some() {
        return None;
    }
    let pname = syntax::name_of(param, ctx.source());
    let s = single_statement(m)?;
    if kind(s) != "expression_statement" {
        return None;
    }
    let a = syntax::first_named_child(s)?;
    if kind(a) != "assignment_expression" || syntax::operator_str(a) != "=" {
        return None;
    }
    let right = syntax::strip_parens(a.field(Field::Right)?);
    if kind(right) != "identifier" || ctx.text(right) != pname {
        return None;
    }
    Some((prop, field_ref(ctx, a.field(Field::Left)?)?))
}

impl<'t> JavaRule<'t> for UnsynchronizedAccessor {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["class_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        // Only direct members are looked at (a text search of the body would rescan nested
        // classes once per level).
        if !ctx
            .members(body)
            .any(|m| kind(m) == "method_declaration" && syntax::has_modifier(m, "synchronized"))
        {
            return;
        }
        let src = ctx.source();
        let mut getters = Vec::new();
        let mut setters = Vec::new();
        for m in ctx.members(body) {
            if kind(m) != "method_declaration" {
                continue;
            }
            let name = syntax::name_of(m, src);
            if name.starts_with("get") || name.starts_with("is") {
                if let Some((p, f)) = getter(ctx, m) {
                    getters.push((m, p, f));
                }
            } else if name.starts_with("set") {
                if let Some((p, f)) = setter(ctx, m) {
                    setters.push((m, p, f));
                }
            }
        }
        if getters.is_empty() || setters.is_empty() || ctx.has_lombok(node) {
            return;
        }
        for &(g, gp, gf) in &getters {
            let Some(&(s, _, _)) = setters.iter().find(|(_, sp, sf)| *sp == gp && *sf == gf) else {
                continue;
            };
            let (g_sync, s_sync) = (
                syntax::has_modifier(g, "synchronized"),
                syntax::has_modifier(s, "synchronized"),
            );
            if g_sync == s_sync
                || syntax::has_modifier(g, "static") != syntax::has_modifier(s, "static")
            {
                continue;
            }
            // The field: declared in this class, neither volatile nor final.
            let field = ctx.members(body).find(|f| {
                kind(*f) == "field_declaration"
                    && syntax::declarators(*f).any(|d| syntax::name_of(d, src) == gf)
            });
            let Some(field) = field else { continue };
            if syntax::has_modifier(field, "volatile") || syntax::has_modifier(field, "final") {
                continue;
            }
            let (unsync, sync) = if g_sync { (s, g) } else { (g, s) };
            let (Some(at), Some(other)) = (unsync.field(Field::Name), sync.field(Field::Name))
            else {
                continue;
            };
            let (un, sy) = (ctx.text(at), ctx.text(other));
            let access = if g_sync { "writes" } else { "reads" };
            ctx.emit(
                at,
                format!("`{un}` {access} `{gf}` without the lock that `{sy}` holds: synchronize both or make the field volatile"),
            )
            .secondary(other, "synchronized here");
        }
    }
}
