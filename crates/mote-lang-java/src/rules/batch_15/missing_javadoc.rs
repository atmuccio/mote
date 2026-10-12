//! `java/missing-javadoc` — a public or protected type, method, constructor or field of a
//! visible type without a documentation comment.

use super::util;
use crate::prelude::*;
// The framework's kind classes: integer comparisons on the rule's hot paths.
use crate::kinds::{k, K};

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["conventions", "documentation"],
    remediation_minutes: 5,
    description: r#"Detects declarations that other code can see but that carry no
documentation comment (`/** … */`): public top-level types, and the public and protected
nested types, methods, constructors and fields of such types (members of interfaces and
annotation types are public unless declared `private`).

**Why it matters.** The visible declarations of a type are its contract. Without
documentation, every caller has to read the implementation to learn what a method expects,
returns, throws or guarantees, and the implementation then becomes the specification: any
change to it may break someone. A short comment stating the purpose and the edge cases is far
cheaper to write than to reconstruct.

**How to fix.** Add a doc comment above the declaration (and above its annotations).

```java
// Noncompliant
public final class RateLimiter {
    public boolean tryAcquire(int permits) { … }
}

// Compliant
/** Limits how often an operation may run, with a fixed number of permits per second. */
public final class RateLimiter {
    /**
     * Takes permits if they are available right now.
     *
     * @param permits how many permits to take; must be positive
     * @return whether the permits were taken
     */
    public boolean tryAcquire(int permits) { … }
}
```

Not reported: test files; members of private, package-private, local and anonymous classes;
methods annotated `@Override` (they inherit the documentation of the method they override);
simple accessors (a getter `getX()` / `isX()` / `hasX()` whose body only returns a field, a
setter `setX(v)` whose body only assigns it, optionally followed by `return this;`); compact
record constructors; and enum constants. A doc comment placed after the annotations counts
here (`java/dangling-doc-comment` reports its position). The rule is off by default and
reports a lot on code that is not documented: it is part of the `conventions` profile."#,
    ..RuleMeta::new(
        "java/missing-javadoc",
        "java",
        "Visible declaration without documentation",
    )
};

/// The rule follows the walk instead of iterating each container's children (iterating a long
/// type body costs more than everything else the rule does): it sees the doc comments and the
/// declarations of a container in source order, as the walk reaches them.
#[derive(Default)]
pub struct MissingJavadoc {
    /// What the last doc comment seen in a container documents: the comment's next named
    /// sibling that is not a comment (its id and start byte).
    documented: Option<(usize, usize)>,
    /// The container asked about last (its id) and whether its members are checked.
    owner: Option<(usize, bool)>,
}

/// How members of a container are visible to other code.
#[derive(Clone, Copy, PartialEq)]
enum Container {
    /// The top level of a file: `public` types only.
    File,
    /// A class, enum or record body: `public` and `protected` members.
    Class,
    /// An interface or annotation body: every member that is not `private`.
    Interface,
}

fn container_of(body: Node<'_>) -> Option<Container> {
    match k(body) {
        K::Program => Some(Container::File),
        K::ClassBody | K::EnumBodyDecls => Some(Container::Class),
        K::InterfaceBody | K::AnnotationTypeBody => Some(Container::Interface),
        _ => None,
    }
}

/// The visibility keyword of a declaration, read in one pass over its modifiers.
#[derive(Clone, Copy, PartialEq)]
enum Access {
    Public,
    Protected,
    Private,
    Package,
}

fn access(decl: Node<'_>) -> Access {
    let Some(m) = syntax::modifiers(decl) else {
        return Access::Package;
    };
    for c in syntax::children(m) {
        if c.is_named() {
            continue;
        }
        match kind(c) {
            "public" => return Access::Public,
            "protected" => return Access::Protected,
            "private" => return Access::Private,
            _ => {}
        }
    }
    Access::Package
}

/// The visibility a member has for other code (`None` when it is not visible).
fn visible_as(access: Access, container: Container) -> Option<&'static str> {
    match (container, access) {
        (_, Access::Public) => Some("public"),
        (Container::Class, Access::Protected) => Some("protected"),
        (Container::Interface, Access::Package) => Some("public"),
        _ => None,
    }
}

fn is_visible(decl: Node<'_>, container: Container) -> bool {
    visible_as(access(decl), container).is_some()
}

/// Whether the type owning `body` is visible from other packages: every enclosing type is a
/// visible member of its container, up to the file. Ancestors alternate between a type
/// declaration and its container (`enum_body` is skipped: enum members sit in its
/// `enum_body_declarations`); anything else (a method, a block, an anonymous class) means a
/// local or anonymous type.
fn owner_is_visible<'t>(ctx: &Ctx<'t>, body: Node<'t>) -> bool {
    let mut ancestors = ctx.ancestors(body).filter(|a| kind(*a) != "enum_body");
    loop {
        let Some(decl) = ancestors.next() else {
            return false;
        };
        if !syntax::is_type_declaration(decl) {
            return false;
        }
        let Some(container) = ancestors.next().and_then(container_of) else {
            return false;
        };
        if !is_visible(decl, container) {
            return false;
        }
        if container == Container::File {
            return true;
        }
    }
}

/// `return field;` or `return this.field;`.
fn returns_field(stmt: Node<'_>) -> bool {
    kind(stmt) == "return_statement"
        && syntax::first_named_child(stmt).is_some_and(|v| is_field_ref(v))
}

fn is_field_ref(node: Node<'_>) -> bool {
    match kind(node) {
        "identifier" => true,
        "field_access" => node.field(Field::Object).is_some_and(|o| kind(o) == "this"),
        _ => false,
    }
}

/// A getter that only returns a field, or a setter that only assigns one (possibly followed by
/// `return this;`).
fn is_simple_accessor<'t>(ctx: &Ctx<'t>, method: Node<'t>) -> bool {
    let name = ctx.name_of(method);
    let after_prefix = |p: &str| {
        name.strip_prefix(p)
            .and_then(|r| r.chars().next())
            .is_some_and(|c| c.is_uppercase() || c.is_ascii_digit() || c == '_')
    };
    // Cheapest first: the name.
    let getter = after_prefix("get") || after_prefix("is") || after_prefix("has");
    let setter = after_prefix("set");
    if !getter && !setter {
        return false;
    }
    // Then the parameters: a getter takes none, a setter one.
    let params = syntax::parameters(method).take(2).count();
    if params != usize::from(setter) {
        return false;
    }
    let Some(body) = method.field(Field::Body) else {
        return false;
    };
    let mut statements = syntax::named_children(body);
    let first = statements.next();
    let second = statements.next();
    if statements.next().is_some() {
        return false;
    }
    if getter {
        return second.is_none() && first.is_some_and(returns_field);
    }
    let assigns = first.is_some_and(|s| {
        kind(s) == "expression_statement"
            && syntax::first_named_child(s).is_some_and(|a| {
                kind(a) == "assignment_expression"
                    && syntax::operator_str(a) == "="
                    && a.field(Field::Left).is_some_and(is_field_ref)
            })
    });
    let tail_ok = second.is_none_or(|s| {
        kind(s) == "return_statement"
            && syntax::first_named_child(s).is_some_and(|v| kind(v) == "this")
    });
    assigns && tail_ok
}

/// What to call a declaration in the message; `None` when it is not checked.
fn describe(decl: Node<'_>) -> Option<&'static str> {
    Some(match k(decl) {
        K::ClassDecl => "class",
        K::InterfaceDecl => "interface",
        K::EnumDecl => "enum",
        K::RecordDecl => "record",
        K::AnnotationTypeDecl => "annotation type",
        K::MethodDecl => "method",
        K::ConstructorDecl => "constructor",
        K::FieldDecl | K::ConstantDecl => "field",
        K::Other if kind(decl) == "annotation_type_element_declaration" => "method",
        _ => return None,
    })
}

/// What the modifiers of a member say, read in one pass: its visibility keyword, an
/// `@Override` annotation, any annotation, and a doc comment inside the modifiers (after an
/// annotation).
struct ModifierFacts<'t> {
    modifiers: Option<Node<'t>>,
    access: Access,
    overrides: bool,
    annotated: bool,
    doc_inside: bool,
}

fn modifier_facts<'t>(ctx: &Ctx<'t>, decl: Node<'t>) -> ModifierFacts<'t> {
    let mut facts = ModifierFacts {
        modifiers: syntax::modifiers(decl),
        access: Access::Package,
        overrides: false,
        annotated: false,
        doc_inside: false,
    };
    let Some(m) = facts.modifiers else {
        return facts;
    };
    for c in syntax::children(m) {
        if !c.is_named() {
            match kind(c) {
                "public" => facts.access = Access::Public,
                "protected" => facts.access = Access::Protected,
                "private" => facts.access = Access::Private,
                _ => {}
            }
            continue;
        }
        match k(c) {
            K::MarkerAnnotation | K::Annotation => {
                facts.annotated = true;
                // The name is part of the text: read it only when the text can hold it.
                let text = ctx.text(c);
                if text == "@Override"
                    || (text.contains("Override")
                        && syntax::annotation_name(c, ctx.source()) == "Override")
                {
                    facts.overrides = true;
                }
            }
            K::BlockComment if util::is_doc_comment(ctx.text(c)) => facts.doc_inside = true,
            _ => {}
        }
    }
    facts
}

/// A doc comment right after the modifiers (`@Deprecated /** … */ void run();`).
fn doc_after_modifiers<'t>(ctx: &Ctx<'t>, decl: Node<'t>, modifiers: Node<'t>) -> bool {
    util::cursor_at(decl, modifiers).is_some_and(|mut cur| {
        while cur.goto_next_sibling() {
            let n = cur.node();
            if !syntax::is_comment(n) {
                return false;
            }
            if kind(n) == "block_comment" && util::is_doc_comment(ctx.text(n)) {
                return true;
            }
        }
        false
    })
}

impl MissingJavadoc {
    /// Whether the members of `container` (the parent of the node being checked) are checked:
    /// the type owning it is visible. Members of one container mostly come one after the
    /// other, so one remembered answer is enough.
    fn checks<'t>(&mut self, ctx: &Ctx<'t>, container: Node<'t>, of: Container) -> bool {
        if of == Container::File {
            return true;
        }
        let id = container.id();
        if let Some((owner, checked)) = self.owner {
            if owner == id {
                return checked;
            }
        }
        let checked = owner_is_visible(ctx, container);
        self.owner = Some((id, checked));
        checked
    }

    /// A doc comment that is a child of a container documents the next named child that is
    /// not a comment (anonymous tokens such as a stray `;` and other comments are skipped).
    fn doc_comment<'t>(&mut self, comment: Node<'t>, ctx: &Ctx<'t>) {
        if !util::is_doc_comment(ctx.text(comment)) {
            return;
        }
        let Some(parent) = ctx.parent(comment) else {
            return;
        };
        let Some(container) = container_of(parent) else {
            return;
        };
        if !self.checks(ctx, parent, container) {
            return;
        }
        // Several comments before one declaration: the first one already found it (only
        // comments and tokens of the container lie between a comment and that declaration).
        if self
            .documented
            .is_some_and(|(_, start)| start >= comment.end_byte())
        {
            return;
        }
        self.documented = util::next_code(parent, comment).map(|n| (n.id(), n.start_byte()));
    }
}

impl<'t> JavaRule<'t> for MissingJavadoc {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "block_comment",
            "class_declaration",
            "interface_declaration",
            "enum_declaration",
            "record_declaration",
            "annotation_type_declaration",
            "method_declaration",
            "annotation_type_element_declaration",
            "constructor_declaration",
            "field_declaration",
            "constant_declaration",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if ctx.is_test() {
            return;
        }
        if k(node) == K::BlockComment {
            self.doc_comment(node, ctx);
            return;
        }
        let Some(what) = describe(node) else {
            return;
        };
        let Some(parent) = ctx.parent(node) else {
            return;
        };
        let Some(container) = container_of(parent) else {
            return;
        };
        if container == Container::File && !syntax::is_type_declaration(node) {
            return;
        }
        if !self.checks(ctx, parent, container) {
            return;
        }
        if self.documented.is_some_and(|(id, _)| id == node.id()) {
            return;
        }
        let facts = modifier_facts(ctx, node);
        let Some(visibility) = visible_as(facts.access, container) else {
            return;
        };
        let is_method = k(node) == K::MethodDecl;
        if facts.doc_inside || (is_method && facts.overrides) {
            return;
        }
        if let Some(m) = facts.modifiers.filter(|_| facts.annotated) {
            if doc_after_modifiers(ctx, node, m) {
                return;
            }
        }
        if is_method && is_simple_accessor(ctx, node) {
            return;
        }
        let name = match k(node) {
            K::FieldDecl | K::ConstantDecl => syntax::declarators(node)
                .next()
                .and_then(|d| d.field(Field::Name)),
            _ => ctx.name_node(node),
        };
        let Some(name) = name else {
            return;
        };
        ctx.emit(name, message(visibility, what, ctx.text(name)));
    }
}

/// "Document the {visibility} {what} `{name}` with a doc comment", built without the
/// formatting machinery (the rule reports a lot on undocumented code).
fn message(visibility: &str, what: &str, name: &str) -> String {
    let mut s = String::with_capacity(36 + visibility.len() + what.len() + name.len());
    s.push_str("Document the ");
    s.push_str(visibility);
    s.push(' ');
    s.push_str(what);
    s.push_str(" `");
    s.push_str(name);
    s.push_str("` with a doc comment");
    s
}
