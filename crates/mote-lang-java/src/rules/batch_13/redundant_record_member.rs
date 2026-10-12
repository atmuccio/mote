//! `java/redundant-record-member` — record members that restate what the compiler generates.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["redundant", "record"],
    remediation_minutes: 2,
    description: r#"Detects members of a `record` that repeat what the compiler already generates:

* an accessor that only returns its component (`public String name() { return name; }`);
* a canonical constructor that only assigns each parameter to its field, and a compact
  constructor with an empty body;
* a `serialVersionUID` field: records are serialized through their components, and the
  serialization mechanism does not compare this value for them.

**Why it matters.** The generated members are exactly these. Writing them out adds lines that
look meaningful, so readers search them for the validation, normalization or defensive copy
they expect a hand-written member to contain — and a later change to a component has more
places to update.

**How to fix.** Remove the member.

```java
// Noncompliant
record Point(int x, int y) {
    Point(int x, int y) {
        this.x = x;
        this.y = y;
    }
    public int x() { return x; }
}

// Compliant
record Point(int x, int y) { }
```

Not reported: members with annotations other than `@Override` (they may be there for a
framework), members documented with a Javadoc comment, accessors or constructors that do
anything else (normalize, validate, copy), and constructors whose parameters carry
annotations."#,
    ..RuleMeta::new(
        "java/redundant-record-member",
        "java",
        "Record member that repeats the generated one",
    )
};

pub struct RedundantRecordMember;

impl<'t> JavaRule<'t> for RedundantRecordMember {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["record_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        let Some(params) = node.field(Field::Parameters) else {
            return;
        };
        let src = ctx.source();
        // Components: (name, type text).
        let mut components: Vec<(&'t str, &'t str)> = Vec::new();
        for p in named_children(params) {
            if kind(p) != "formal_parameter" {
                // Varargs components and anything unusual: leave the record alone.
                return;
            }
            let (Some(n), Some(t)) = (p.field(Field::Name), p.field(Field::Type)) else {
                return;
            };
            components.push((ctx.text(n), ctx.text(t)));
        }
        for m in ctx.members(body) {
            match kind(m) {
                "method_declaration" => check_accessor(ctx, m, &components),
                "constructor_declaration" => check_canonical(ctx, m, &components),
                "compact_constructor_declaration" => {
                    if only_override(ctx, m) && !util::has_leading_comment(ctx, m) {
                        if let Some(b) = m.field(Field::Body) {
                            if b.named_child_count() == 0 {
                                let at = m.field(Field::Name).unwrap_or(m);
                                ctx.emit(at, "Remove this empty compact constructor: the record already has this constructor");
                            }
                        }
                    }
                }
                "field_declaration" => {
                    if !syntax::has_modifier(m, "static") || !only_override(ctx, m) {
                        continue;
                    }
                    for d in syntax::declarators(m) {
                        if let Some(n) = d.field(Field::Name) {
                            if ctx.text(n) == "serialVersionUID"
                                && !util::has_leading_comment(ctx, m)
                            {
                                ctx.emit(n, "Remove `serialVersionUID`: records are serialized by their components and never compare it");
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        let _ = src;
    }
}

/// No annotation other than `@Override`.
fn only_override<'t>(ctx: &Ctx<'t>, decl: Node<'t>) -> bool {
    util::annotation_names(ctx, decl).all(|(_, n)| n == "Override")
}

fn check_accessor<'t>(ctx: &Ctx<'t>, m: Node<'t>, components: &[(&'t str, &'t str)]) {
    let src = ctx.source();
    let name = syntax::name_of(m, src);
    let Some(&(_, ctype)) = components.iter().find(|(n, _)| *n == name) else {
        return;
    };
    if syntax::parameters(m).next().is_some() || m.field(Field::TypeParameters).is_some() {
        return;
    }
    let Some(ty) = m.field(Field::Type) else {
        return;
    };
    if compact(ctx.text(ty)) != compact(ctype) || !syntax::has_modifier(m, "public") {
        return;
    }
    let Some(body) = m.field(Field::Body) else {
        return;
    };
    let Some(stmt) = util::single_statement(body) else {
        return;
    };
    if kind(stmt) != "return_statement" {
        return;
    }
    let Some(e) = syntax::first_named_child(stmt).map(syntax::strip_parens) else {
        return;
    };
    let returns_field = match kind(e) {
        "identifier" => ctx.text(e) == name,
        "field_access" => {
            e.field(Field::Object).is_some_and(|o| kind(o) == "this")
                && e.field(Field::Field).is_some_and(|f| ctx.text(f) == name)
        }
        _ => false,
    };
    if !returns_field || !only_override(ctx, m) || util::has_leading_comment(ctx, m) {
        return;
    }
    let Some(n) = m.field(Field::Name) else {
        return;
    };
    ctx.emit(
        n,
        format!(
            "Remove this accessor: the record already generates `{name}()` returning the field"
        ),
    );
}

fn check_canonical<'t>(ctx: &Ctx<'t>, c: Node<'t>, components: &[(&'t str, &'t str)]) {
    let params: Vec<Node<'t>> = syntax::parameters(c).collect();
    if params.len() != components.len() {
        return;
    }
    for (p, (cname, ctype)) in params.iter().zip(components) {
        if kind(*p) != "formal_parameter" || syntax::annotations(*p).next().is_some() {
            return;
        }
        let (Some(n), Some(t)) = (p.field(Field::Name), p.field(Field::Type)) else {
            return;
        };
        if ctx.text(n) != *cname || compact(ctx.text(t)) != compact(ctype) {
            return;
        }
    }
    let Some(body) = c.field(Field::Body) else {
        return;
    };
    // Every statement is `this.x = x;`, each component exactly once.
    let mut assigned: Vec<&str> = Vec::new();
    for s in named_children(body) {
        if syntax::is_comment(s) {
            return; // an explanation: leave it
        }
        if kind(s) != "expression_statement" {
            return;
        }
        let Some(a) = syntax::first_named_child(s) else {
            return;
        };
        if kind(a) != "assignment_expression" || syntax::operator_str(a) != "=" {
            return;
        }
        let (Some(l), Some(r)) = (a.field(Field::Left), a.field(Field::Right)) else {
            return;
        };
        let target = (kind(l) == "field_access"
            && l.field(Field::Object).is_some_and(|o| kind(o) == "this"))
        .then(|| l.field(Field::Field))
        .flatten()
        .map(|f| ctx.text(f));
        let Some(target) = target else {
            return;
        };
        if kind(r) != "identifier" || ctx.text(r) != target || assigned.contains(&target) {
            return;
        }
        assigned.push(target);
    }
    if assigned.len() != components.len()
        || !only_override(ctx, c)
        || util::has_leading_comment(ctx, c)
    {
        return;
    }
    let at = c.field(Field::Name).unwrap_or(c);
    ctx.emit(
        at,
        "Remove this constructor: it only assigns the fields, as the generated canonical constructor does",
    );
}

fn compact(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}
