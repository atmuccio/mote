//! `java/foreach-element-cast` — a for-each variable declared with a wider type than the
//! elements and cast back to the element type at once.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Incubating,
    tags: &["clarity", "casts"],
    remediation_minutes: 2,
    description: r#"Detects a for-each loop whose variable is declared with a wider type than the
known element type of the array or collection, when the loop body starts by casting the
variable back to that element type.

**Why it matters.** The cast only undoes the widening of the declaration: the elements already
have the type. The detour adds a line, a second name for the same element, and a cast that
readers must check and that hides a real mistake if the collection's type changes later.

**How to fix.** Declare the loop variable with the element type and drop the cast.

```java
// Noncompliant
for (Object o : names) {          // List<String> names
    String name = (String) o;
    print(name);
}

// Compliant
for (String name : names) {
    print(name);
}
```

Only element types the resolver knows are considered (arrays, and `Iterable` types with a
known type argument); raw collections, where the cast is needed, are not reported. Not
reported either: casts to a type other than the element type (a narrowing beyond it), and
casts that are not the first statement of the body."#,
    ..RuleMeta::new(
        "java/foreach-element-cast",
        "java",
        "For-each variable widened and cast back",
    )
};

pub struct ForeachElementCast;

impl<'t> JavaRule<'t> for ForeachElementCast {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["enhanced_for_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let (Some(var), Some(body), Some(decl_type), Some(iterable)) = (
            node.field(Field::Name),
            node.field(Field::Body),
            node.field(Field::Type),
            node.field(Field::Value),
        ) else {
            return;
        };
        if kind(body) != "block" || kind(var) != "identifier" {
            return;
        }
        // The first statement: `T x = (T) var;`.
        let Some(first) = syntax::first_named_child(body) else {
            return;
        };
        if kind(first) != "local_variable_declaration" {
            return;
        }
        let mut decls = syntax::declarators(first);
        let (Some(d), None) = (decls.next(), decls.next()) else {
            return;
        };
        let Some(cast) = d.field(Field::Value).map(syntax::strip_parens) else {
            return;
        };
        if kind(cast) != "cast_expression" {
            return;
        }
        let (Some(cast_type), Some(value)) = (cast.field(Field::Type), cast.field(Field::Value))
        else {
            return;
        };
        let value = syntax::strip_parens(value);
        let var_name = ctx.text(var);
        if kind(value) != "identifier" || ctx.text(value) != var_name {
            return;
        }
        // Resolver work last.
        let Some(iter_t) = ctx.type_of(iterable) else {
            return;
        };
        let element = if iter_t.is_array() {
            iter_t.element_type().cloned()
        } else {
            ctx.as_supertype(&iter_t, "java.lang.Iterable")
                .and_then(|t| t.type_arg(0).cloned())
        };
        let Some(element) = element.filter(|e| e.is_fully_known() && e.is_reference()) else {
            return;
        };
        let (Some(declared), Some(cast_to)) =
            (ctx.resolve_type(decl_type), ctx.resolve_type(cast_type))
        else {
            return;
        };
        if cast_to != element || declared == element {
            return;
        }
        if !ctx.is_subtype_of(&element, declared.fqn().unwrap_or("")) {
            return;
        }
        ctx.emit(
            decl_type,
            format!(
                "Declare the loop variable as `{}` and drop the cast: the elements already have that type",
                ctx.text(cast_type)
            ),
        )
        .secondary(cast, "cast back to the element type");
    }
}
