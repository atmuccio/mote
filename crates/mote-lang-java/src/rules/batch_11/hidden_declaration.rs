//! `java/hidden-declaration` — declarations that hide another declaration of the same file: a
//! local variable named like a field of its class, a field hiding a parent class's field, a
//! type parameter hiding an enclosing one.

use super::util::{self, Walk};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Incubating,
    default_enabled: false,
    tags: &["confusing", "naming", "conventions"],
    remediation_minutes: 5,
    description: r#"Detects declarations that hide another declaration of the same file, so that one
name means two things in the same place:

* a local variable of an instance method named like a field of its class;
* an instance field declared again, under the same name, in a subclass of a class of the same
  file that already has it (non-private);
* a type parameter of an instance method, constructor or inner class named like a type
  parameter of the enclosing class.

**Why it matters.** Inside the scope of the hiding declaration the plain name refers to the
new one, while readers (and the author a few weeks later) expect the other: an assignment
meant for the field only changes the local, a subclass method sees one copy of the state and
the parent's methods the other, and `T` in the method signature is not the class's `T`.

**How to fix.** Rename one of the two declarations (or, for a field, remove the duplicate and
use the parent's).

```java
// Noncompliant
class Order {
    private int total;
    void add(Line line) {
        int total = line.price() * line.quantity();   // hides the field
        …
    }
}

// Compliant
class Order {
    private int total;
    void add(Line line) {
        int lineTotal = line.price() * line.quantity();
        …
    }
}
```

Not reported: parameters (constructor, setter, builder and factory parameters named like the
field they initialize are the normal idiom), locals in constructors, static methods, lambdas,
anonymous classes and test code, locals holding a copy of the object's state
(`Thread worker = this.worker;`, the usual way to read a volatile field once, or
`Node root = getRoot();`) or whose value is later stored in the field
(`this.total = total`, or through the class's own setter: `setTotal(total)`), static fields (each class may define its own constants or logger),
static methods and static nested classes reusing a type parameter name (they cannot see the
enclosing one), types named like the type they extend (nested `Builder extends
Parent.Builder` classes and drop-in replacements of a library class are deliberate), and
hiding across files, which needs the other files' types.

Off by default: names shared between a local and a field are a style many code bases
accept."#,
    ..RuleMeta::new(
        "java/hidden-declaration",
        "java",
        "Declaration hides another one",
    )
};

#[derive(Default)]
pub struct HiddenDeclaration<'t> {
    /// Class body id → names of the fields declared in it, with whether they are static.
    fields: FxHashMap<usize, Vec<(&'t str, bool)>>,
    /// Class declarations of the file (for fields hiding a parent's field).
    classes: Vec<Node<'t>>,
    /// Names of every field declared in the file (a cheap first test), built on first use.
    all_fields: Option<FxHashSet<&'t str>>,
}

/// The fields declared directly in `body` (record components included for records).
fn field_names<'t>(ctx: &Ctx<'t>, class: Node<'t>, body: Node<'t>) -> Vec<(&'t str, bool)> {
    let src = ctx.source();
    let mut out = Vec::new();
    for m in ctx.members(body) {
        if kind(m) != "field_declaration" {
            continue;
        }
        let is_static = syntax::has_modifier(m, "static");
        for d in syntax::declarators(m) {
            out.push((syntax::name_of(d, src), is_static));
        }
    }
    if kind(class) == "record_declaration" {
        if let Some(params) = class.field(Field::Parameters) {
            for p in named_children(params) {
                if let Some(n) = p.field(Field::Name) {
                    out.push((ctx.text(n), false));
                }
            }
        }
    }
    out
}

/// Type parameter names of a declaration.
fn type_params<'t>(ctx: &Ctx<'t>, decl: Node<'t>) -> Vec<(&'t str, Node<'t>)> {
    let Some(tp) = decl.field(Field::TypeParameters) else {
        return Vec::new();
    };
    named_children(tp)
        .filter(|p| kind(*p) == "type_parameter")
        .filter_map(|p| {
            let name = named_children(p).find(|c| kind(*c) == "type_identifier")?;
            Some((ctx.text(name), name))
        })
        .collect()
}

/// `foo()` or `this.foo()` without arguments: a read of this object's own state.
fn is_own_accessor_call(v: Node<'_>) -> bool {
    kind(v) == "method_invocation"
        && v.field(Field::Object).is_none_or(|o| kind(o) == "this")
        && syntax::argument_count(v) == 0
}

/// Whether `name` is assigned to `this.name` somewhere in `body`, or the declarator's value
/// reads `this.name`.
fn stores_or_copies_field<'t>(ctx: &Ctx<'t>, body: Node<'t>, name: &str) -> bool {
    util::visit(body, |n| {
        if kind(n) == "field_access"
            && n.field(Field::Object).is_some_and(|o| kind(o) == "this")
            && n.field(Field::Field).is_some_and(|f| ctx.text(f) == name)
        {
            Walk::Stop
        } else {
            Walk::Continue
        }
    })
}

/// Whether `method_body` passes the local `name` to the class's own setter of the field of that
/// name (`setFontName(fontName)` / `this.setFontName(fontName)`), where the setter is declared in
/// `class_body` with one parameter and assigns `this.<name>`.
fn passes_to_own_setter<'t>(
    ctx: &Ctx<'t>,
    method_body: Node<'t>,
    class_body: Node<'t>,
    name: &str,
) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    let setter = format!("set{}{}", first.to_uppercase(), chars.as_str());
    if !ctx.text(method_body).contains(&format!("{setter}(")) {
        return false;
    }
    let declared = ctx.members(class_body).any(|m| {
        kind(m) == "method_declaration"
            && syntax::name_of(m, ctx.source()) == setter
            && syntax::parameters(m).count() == 1
            && m.field(Field::Body)
                .is_some_and(|b| stores_or_copies_field(ctx, b, name))
    });
    if !declared {
        return false;
    }
    util::visit(method_body, |n| {
        let passes = kind(n) == "method_invocation"
            && ctx.call_name(n) == Some(setter.as_str())
            && n.field(Field::Object).is_none_or(|o| kind(o) == "this")
            && syntax::argument_count(n) == 1
            && syntax::arguments(n)
                .next()
                .is_some_and(|a| kind(a) == "identifier" && ctx.text(a) == name);
        if passes {
            Walk::Stop
        } else {
            Walk::Continue
        }
    })
}

impl<'t> HiddenDeclaration<'t> {
    fn check_local(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if ctx.is_test() {
            return;
        }
        let all_fields = self.all_fields.get_or_insert_with(|| {
            let mut s = FxHashSet::default();
            collect_field_names(ctx, ctx.root(), &mut s, 0);
            s
        });
        let src = ctx.source();
        if !syntax::declarators(node).any(|d| all_fields.contains(syntax::name_of(d, src))) {
            return;
        }
        // The function directly holding the declaration must be an instance method.
        let Some(func) = ctx.enclosing_function(node) else {
            return;
        };
        if kind(func) != "method_declaration" || syntax::has_modifier(func, "static") {
            return;
        }
        let Some(class) = ctx.enclosing_class(node) else {
            return;
        };
        if !matches!(
            kind(class),
            "class_declaration" | "enum_declaration" | "record_declaration"
        ) {
            return;
        }
        let Some(body) = syntax::type_body(class) else {
            return;
        };
        let fields = self
            .fields
            .entry(body.id())
            .or_insert_with(|| field_names(ctx, class, body));
        if fields.is_empty() {
            return;
        }
        for d in syntax::declarators(node) {
            let name = syntax::name_of(d, src);
            if !fields
                .iter()
                .any(|(f, is_static)| *f == name && !*is_static)
            {
                continue;
            }
            // `Foo foo = getFoo();`: a copy of this object's state read through its own
            // accessor, like `this.foo`.
            if d.field(Field::Value)
                .map(syntax::strip_parens)
                .is_some_and(|v| is_own_accessor_call(v))
            {
                continue;
            }
            let Some(method_body) = func.field(Field::Body) else {
                continue;
            };
            if ctx.text(method_body).contains("this.")
                && stores_or_copies_field(ctx, method_body, name)
            {
                continue;
            }
            // `setFontName(fontName)`: stored in the field through the class's own setter.
            if passes_to_own_setter(ctx, method_body, body, name) {
                continue;
            }
            let Some(name_node) = d.field(Field::Name) else {
                continue;
            };
            ctx.emit(
                name_node,
                format!("Rename this local variable: it hides the field `{name}` of its class"),
            );
        }
    }

    fn check_type(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        // An inner (non-static) class reusing a type parameter of an enclosing class.
        if kind(node) == "class_declaration"
            && node.field(Field::TypeParameters).is_some()
            && !syntax::has_modifier(node, "static")
        {
            self.check_type_params(node, ctx);
        }
        if kind(node) == "class_declaration" && node.field(Field::Superclass).is_some() {
            self.classes.push(node);
        }
    }

    /// Type parameters of `decl` (an instance method, constructor or inner class) named like
    /// one of the directly enclosing class.
    fn check_type_params(&mut self, decl: Node<'t>, ctx: &Ctx<'t>) {
        let own = type_params(ctx, decl);
        if own.is_empty() {
            return;
        }
        // The enclosing class (for a class: the one around it).
        let Some(outer) = ctx
            .ancestors(decl)
            .find(|a| syntax::is_type_declaration(*a) || kind(*a) == "object_creation_expression")
        else {
            return;
        };
        if kind(outer) != "class_declaration" {
            return;
        }
        let outer_params = type_params(ctx, outer);
        for (name, at) in own {
            if outer_params.iter().any(|(o, _)| *o == name) {
                ctx.emit(
                    at,
                    format!(
                        "Rename this type parameter: it hides the type parameter `{name}` of the \
                         enclosing class"
                    ),
                );
            }
        }
    }
}

impl<'t> JavaRule<'t> for HiddenDeclaration<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "local_variable_declaration",
            "class_declaration",
            "interface_declaration",
            "method_declaration",
            "constructor_declaration",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        match kind(node) {
            "local_variable_declaration" => self.check_local(node, ctx),
            "class_declaration" | "interface_declaration" => self.check_type(node, ctx),
            "method_declaration" | "constructor_declaration"
                // Only generic declarations have type parameters to compare (cheapest first).
                if node.field(Field::TypeParameters).is_some()
                    && !syntax::has_modifier(node, "static")
                => {
                    self.check_type_params(node, ctx);
                }
            _ => {}
        }
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        // Instance fields declared again in a subclass of a class of this file.
        let src = ctx.source();
        for &class in &self.classes {
            let Some(sc) = class
                .field(Field::Superclass)
                .and_then(syntax::first_named_child)
            else {
                continue;
            };
            let Some(parent_fqn) = util::type_fqn(ctx, sc) else {
                continue;
            };
            let Some(parent) = util::find_type_decl(ctx, ctx.root(), &parent_fqn, 0)
                .filter(|p| kind(*p) == "class_declaration")
            else {
                continue;
            };
            let (Some(pbody), Some(cbody)) = (syntax::type_body(parent), syntax::type_body(class))
            else {
                continue;
            };
            let parent_fields: Vec<&str> = ctx
                .members(pbody)
                .filter(|m| {
                    kind(*m) == "field_declaration"
                        && !syntax::has_modifier(*m, "static")
                        && !syntax::has_modifier(*m, "private")
                })
                .flat_map(|m| syntax::declarators(m).map(|d| syntax::name_of(d, src)))
                .collect();
            if parent_fields.is_empty() {
                continue;
            }
            for m in ctx.members(cbody) {
                if kind(m) != "field_declaration" || syntax::has_modifier(m, "static") {
                    continue;
                }
                for d in syntax::declarators(m) {
                    let name = syntax::name_of(d, src);
                    if parent_fields.contains(&name) {
                        if let Some(n) = d.field(Field::Name) {
                            ctx.emit(
                                n,
                                format!(
                                    "Rename or remove this field: it hides the field `{name}` \
                                     of the parent class"
                                ),
                            );
                        }
                    }
                }
            }
        }
    }
}

/// Field names (and record components) of the type declarations of `scope`, recursively.
fn collect_field_names<'t>(
    ctx: &Ctx<'t>,
    scope: Node<'t>,
    out: &mut FxHashSet<&'t str>,
    depth: u32,
) {
    if depth > 6 {
        return;
    }
    let src = ctx.source();
    let member = |m: Node<'t>, out: &mut FxHashSet<&'t str>| {
        if kind(m) == "field_declaration" {
            for d in syntax::declarators(m) {
                out.insert(syntax::name_of(d, src));
            }
        } else if syntax::is_type_declaration(m) {
            collect_field_names(ctx, m, out, depth + 1);
        }
    };
    if kind(scope) == "program" {
        for m in named_children(scope) {
            member(m, out);
        }
        return;
    }
    if kind(scope) == "record_declaration" {
        if let Some(params) = scope.field(Field::Parameters) {
            for p in named_children(params) {
                if let Some(n) = p.field(Field::Name) {
                    out.insert(ctx.text(n));
                }
            }
        }
    }
    if let Some(body) = syntax::type_body(scope) {
        for m in ctx.members(body) {
            member(m, out);
        }
    }
}
