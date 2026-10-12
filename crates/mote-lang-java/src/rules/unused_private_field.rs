//! `java/unused-private-field` — a file-level rule keeping `Node<'t>` / `&'t str` state.

use crate::kinds::{k, K};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Medium,
    tags: &["dead-code"],
    remediation_minutes: 5,
    description: r#"Detects `private` fields that are never read anywhere in the file: either not
referenced at all, or only assigned (`this.cache = build();`, `count++;`) and never read.

**Why it matters.** A private field can only be used from its own compilation unit, so if
nothing in the file mentions it, it is dead code: it costs memory, misleads readers into
thinking it matters, and often signals an unfinished refactoring or a bug (a value computed and
stored but never used).

**How to fix.** Remove the field (and whatever computes its value), or use it.

```java
// Noncompliant
class Service {
    private final Clock clock = Clock.systemUTC();   // never read
}

// Compliant
class Service {
    private final Clock clock = Clock.systemUTC();
    Instant now() { return clock.instant(); }
}
```

Not reported: `serialVersionUID` / `serialPersistentFields`, annotated fields (injection,
serialization, persistence frameworks read them reflectively — annotations that only document
the field, such as `@Nullable`, `@Deprecated` or `@GuardedBy`, don't count), fields of
Lombok-annotated classes or with Lombok accessors, and fields whose name appears as a string
literal (reflection). A field that is assigned but never read is not reported in a class that
is annotated, declares `Serializable`/`Externalizable`, or has a field with such an
annotation (a persistence or serialization framework may read its fields). Any identifier
with the field's name counts as a read, so a local variable of the same name that is read
keeps the field quiet."#,
    ..RuleMeta::new("java/unused-private-field", "java", "Unused private field")
};

#[derive(Default)]
pub struct UnusedPrivateField<'t> {
    /// Whether the source mentions `private` at all (computed on first use): without it no
    /// field is a candidate, and nothing needs recording.
    any_private: Option<bool>,
    /// Candidate fields: (name identifier, name, whether a write-only use may be reported).
    fields: Vec<(Node<'t>, &'t str, bool)>,
    /// Reads: every identifier / string literal seen, except field declaration names and the
    /// writes below. Only the candidates' names are looked up, in `finish`.
    uses: Vec<&'t str>,
    /// Identifiers only assigned to (`x = …`, `this.x = …`, `x++;`).
    writes: Vec<&'t str>,
    /// Per class (node id): whether its write-only fields may be reported.
    classes: FxHashMap<usize, bool>,
}

/// Annotations that document a field without implying reflective access.
fn is_documenting_annotation(ann: Node<'_>, src: &str) -> bool {
    match syntax::annotation_name(ann, src) {
        "Nullable" | "NonNull" | "Nonnull" | "CheckForNull" | "Deprecated"
        | "VisibleForTesting" | "GuardedBy" => true,
        "SuppressWarnings" => {
            let args = ann
                .field(Field::Arguments)
                .map_or("", |a| syntax::node_text(a, src));
            !args.contains("unused") && !args.contains("\"all\"")
        }
        _ => false,
    }
}

/// How an identifier is used: an assignment target whose value is discarded is a write.
fn is_write(
    ident: Node<'_>,
    parent: Node<'_>,
    grand: Option<Node<'_>>,
    great: Option<Node<'_>>,
) -> bool {
    // The node that holds the variable: `x` itself, or `this.x` / `other.x`.
    let (target, outer, outer2) = if k(parent) == K::FieldAccess {
        // Only the field of `a.x` is written, and only by an assignment or update around it
        // (the object `a` has the field access itself as its outer node).
        let Some(g) =
            grand.filter(|g| matches!(k(*g), K::AssignmentExpression | K::UpdateExpression))
        else {
            return false;
        };
        if parent
            .field(Field::Field)
            .is_none_or(|f| f.id() != ident.id())
        {
            return false;
        }
        (parent, g, great)
    } else {
        (ident, parent, grand)
    };
    let discarded = || outer2.is_some_and(|s| k(s) == K::ExpressionStatement);
    match k(outer) {
        K::AssignmentExpression => {
            outer
                .field(Field::Left)
                .is_some_and(|l| l.id() == target.id())
                && (syntax::operator_str(outer) == "=" || discarded())
        }
        K::UpdateExpression => discarded(),
        _ => false,
    }
}

/// `this.x = x`: the right-hand `x` is the parameter (or the field copied onto itself).
fn is_shadow_copy<'t>(ident: Node<'t>, parent: Node<'t>, src: &str) -> bool {
    kind(parent) == "assignment_expression"
        && syntax::operator_str(parent) == "="
        && parent
            .field(Field::Right)
            .is_some_and(|r| r.id() == ident.id())
        && parent.field(Field::Left).is_some_and(|l| {
            kind(l) == "field_access"
                && l.field(Field::Field)
                    .is_some_and(|f| syntax::node_text(f, src) == syntax::node_text(ident, src))
        })
}

impl<'t> JavaRule<'t> for UnusedPrivateField<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["field_declaration", "identifier", "string_literal"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        match kind(node) {
            "identifier" => {
                // Package and import names are not uses.
                if node.start_byte() < ctx.header_end() || !self.any_private(ctx) {
                    return;
                }
                // Runs on every identifier: the parent's kind is read once, and the
                // grandparents only for the shapes that need them.
                let mut up = ctx.ancestors(node);
                if let Some(p) = up.next() {
                    let kp = k(p);
                    // Declaring occurrences are not reads: of a field, or of another variable
                    // (parameters, locals, …).
                    let declares = match kp {
                        K::FormalParameter
                        | K::CatchFormalParameter
                        | K::EnhancedForStatement
                        | K::Resource
                        | K::VariableDeclarator => {
                            p.field(Field::Name).is_some_and(|n| n.id() == node.id())
                        }
                        K::InferredParameters => true,
                        K::Lambda => p
                            .field(Field::Parameters)
                            .is_some_and(|n| n.id() == node.id()),
                        _ => false,
                    };
                    if declares {
                        return;
                    }
                    if matches!(
                        kp,
                        K::FieldAccess | K::AssignmentExpression | K::UpdateExpression
                    ) {
                        let (gp, ggp) = (up.next(), up.next());
                        if is_write(node, p, gp, ggp) {
                            self.writes.push(ctx.text(node));
                            return;
                        }
                        if kp == K::AssignmentExpression && is_shadow_copy(node, p, ctx.source()) {
                            return;
                        }
                    }
                }
                self.uses.push(ctx.text(node));
            }
            "string_literal" => {
                if !self.any_private(ctx) {
                    return;
                }
                if let Some(s) = syntax::string_literal_content(node, ctx.source()) {
                    if !s.is_empty() && s.len() < 64 {
                        self.uses.push(s);
                    }
                }
            }
            _ => {
                let src = ctx.source();
                if !syntax::has_modifier(node, "private")
                    || syntax::annotations(node).any(|a| !is_documenting_annotation(a, src))
                {
                    return;
                }
                if ctx.field_has_generated_accessors(node) {
                    return;
                }
                let class = ctx.enclosing_class(node);
                if class.is_some_and(|c| ctx.has_lombok(c)) {
                    return;
                }
                let write_only_ok = class.is_some_and(|c| self.write_only_reportable(c, ctx));
                for d in syntax::declarators(node) {
                    let Some(name) = d.field(Field::Name) else {
                        continue;
                    };
                    let text = ctx.text(name);
                    if matches!(text, "serialVersionUID" | "serialPersistentFields" | "_") {
                        continue;
                    }
                    self.fields.push((name, text, write_only_ok));
                }
            }
        }
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        if self.fields.is_empty() {
            return;
        }
        // Per candidate name: (read somewhere, written somewhere).
        let mut seen: FxHashMap<&str, (bool, bool)> = self
            .fields
            .iter()
            .map(|(_, text, _)| (*text, (false, false)))
            .collect();
        for u in &self.uses {
            if let Some(s) = seen.get_mut(u) {
                s.0 = true;
            }
        }
        for w in &self.writes {
            if let Some(s) = seen.get_mut(w) {
                s.1 = true;
            }
        }
        for (name, text, write_only_ok) in &self.fields {
            let (read, written) = seen.get(text).copied().unwrap_or((false, false));
            if read {
                continue;
            }
            if !written {
                ctx.emit(*name, format!("Remove this unused private field `{text}`"));
            } else if *write_only_ok {
                ctx.emit(
                    *name,
                    format!("Remove the private field `{text}`: it is assigned but never read"),
                );
            }
        }
    }
}

impl<'t> UnusedPrivateField<'t> {
    /// Whether the source contains `private` (a candidate field needs the modifier).
    fn any_private(&mut self, ctx: &Ctx<'t>) -> bool {
        *self
            .any_private
            .get_or_insert_with(|| ctx.source().contains("private"))
    }

    /// Whether fields of `class` that are only assigned may be reported: the class carries no
    /// annotation, doesn't declare `Serializable`/`Externalizable`, and none of its fields has
    /// an annotation other than documenting ones (a sign of a persistence or serialization
    /// framework reading fields reflectively). Cached per class.
    fn write_only_reportable(&mut self, class: Node<'t>, ctx: &Ctx<'t>) -> bool {
        if let Some(&ok) = self.classes.get(&class.id()) {
            return ok;
        }
        let src = ctx.source();
        let annotated = syntax::annotations(class).next().is_some();
        let serializable = class.field(Field::Interfaces).is_some_and(|i| {
            let t = syntax::node_text(i, src);
            t.contains("Serializable") || t.contains("Externalizable")
        });
        let framework_fields = syntax::type_body(class).is_some_and(|body| {
            ctx.members(body)
                .filter(|m| kind(*m) == "field_declaration")
                .any(|f| {
                    syntax::annotations(f).any(|a| {
                        !is_documenting_annotation(a, src)
                            && syntax::annotation_name(a, src) != "SuppressWarnings"
                    })
                })
        });
        let ok = !annotated && !serializable && !framework_fields;
        self.classes.insert(class.id(), ok);
        ok
    }
}
