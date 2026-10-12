//! `js/no-unused-private-class-members`

use crate::prelude::*;

pub struct NoUnusedPrivateClassMembers;

pub static NO_UNUSED_PRIVATE_CLASS_MEMBERS: RuleMeta = RuleMeta {
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["unused", "dead-code"],
    remediation_minutes: 5,
    description: r#"Flags private class members that nothing uses: `#name` fields, methods and accessors,
and TypeScript `private` fields, methods and accessors.

**Why:** a private member can only be reached from inside its class, so when the class never uses
it, no other code can either. It is dead code: it costs readers time, keeps imports and
dependencies alive, and a field that is written but never read suggests that some logic was lost in
an edit.

**Fix:** delete the member (and whatever only exists to feed it), or use it where it was meant to
be used.

```ts
class Cache {
  #hits = 0;                           // flagged: assigned below but never read
  #evict() { /* … */ }                 // flagged: never called
  private legacyKey = 'v1';            // flagged: never used
  store(key: string) { this.#hits = 1; }
}

class Cache {
  #hits = 0;
  get hits() { return this.#hits; }    // compliant: read
}
```

A `#name` field counts as used only when it is read: assigning it alone keeps it flagged, since
`#name` fields are invisible to serialization and reflection. A TypeScript `private` member counts
as used when it is accessed at all, or when its name appears anywhere in the file as a property
access, a string, or a destructured key (other instances, `this['name']`, test hooks). Not
reported: decorated members (framework hooks), constructor parameter properties (dependency
injection declares dependencies with them, and a dependency may be requested only to be created),
fields initialized by a function called on the spot (a substitute for a static block), TypeScript
`private` members of classes decorated as components or directives (templates may use them),
TypeScript `private` members of classes that access `this` with computed keys, `declare` fields,
constructors, ambient declarations, and brand fields that are never assigned: a field such as
`private __brand: void` or `#brand!: never` exists only to make the class nominally distinct for
the type checker (TypeScript treats classes with private members as compatible only with their
own subclasses) and is never meant to be read. A `_`/`$`-prefixed name ending in `brand` counts
as a brand; a plain `brand` only when it is declared with a type and no value.

Known limitation: TypeScript's `private` is checked at compile time only, and bracket access
(`module['preload']()`) is allowed to bypass it, including from other files (typically a test or a
sibling class of the same package). The rule reads one file at a time and cannot see such a use:
suppress the finding on that member, or make the member public (`/** @internal */`) if other
files are meant to call it.
"#,
    ..RuleMeta::new(
        "js/no-unused-private-class-members",
        "js",
        "Private class members should be used",
    )
};

#[derive(Clone, Copy, PartialEq)]
enum MemberKind {
    /// A field or `accessor` field: plain writes do not count as uses (for `#name` fields).
    Field,
    /// A method or getter/setter: any access is a use.
    Method,
}

struct Candidate {
    name: String,
    hash: bool,
    kind: MemberKind,
    span: Span,
    class: NodeId,
    /// A nominal-typing brand field ([`is_brand`]): not reported unless something assigns it.
    brand: bool,
}

/// Private members found in the file's classes.
#[derive(Default)]
struct Candidates {
    list: Vec<Candidate>,
}

/// What the file does with each candidate name (indexed like the file's [`helpers::NameSet`];
/// `[plain name, #name]`).
struct Uses {
    /// Read somewhere (or accessed in a way that is not a plain write).
    read: Vec<[bool; 2]>,
    /// Written with plain `=`.
    written: Vec<[bool; 2]>,
    /// Mentioned in strings or destructuring keys.
    mentioned: Vec<bool>,
}

impl JsRule for NoUnusedPrivateClassMembers {
    fn meta(&self) -> &'static RuleMeta {
        &NO_UNUSED_PRIVATE_CLASS_MEMBERS
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.file_contains(AstType::Class)
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::Class]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::Class(class) = node.kind() else {
            return;
        };
        if class.declare || ctx.in_ambient_context(node) {
            return;
        }
        let mut found = Vec::new();
        collect_candidates(node.id(), class, &mut found);
        ctx.state::<Candidates>().list.extend(found);
    }

    fn run_once<'a>(&self, ctx: &mut Ctx<'a>) {
        let candidates = std::mem::take(&mut ctx.state::<Candidates>().list);
        if candidates.is_empty() {
            return;
        }
        let names = helpers::NameSet::new(candidates.iter().map(|c| c.name.as_str()));
        let mut uses = Uses {
            read: vec![[false; 2]; names.len()],
            written: vec![[false; 2]; names.len()],
            mentioned: vec![false; names.len()],
        };
        let mut dynamic_classes: FxHashSet<NodeId> = FxHashSet::default();
        // Candidates already known not to be reported (the flags only ever get set): once all
        // are, the rest of the file need not be read.
        let mut by_name: Vec<Vec<usize>> = vec![Vec::new(); names.len()];
        for (k, c) in candidates.iter().enumerate() {
            if let Some(i) = names.get(&c.name) {
                by_name[i].push(k);
            }
        }
        let mut settled = vec![false; candidates.len()];
        let mut unsettled = candidates.len();
        for node in ctx.nodes().iter() {
            // The name whose flags this node may have set.
            let touched = match node.kind() {
                AstKind::PrivateFieldExpression(m) => {
                    let i = names.get(m.field.name.as_str());
                    if let Some(i) = i {
                        record(ctx, node, i, true, &mut uses);
                    }
                    i
                }
                AstKind::PrivateInExpression(e) => {
                    let i = names.get(e.left.name.as_str());
                    if let Some(i) = i {
                        uses.read[i][1] = true;
                    }
                    i
                }
                AstKind::StaticMemberExpression(m) => {
                    let i = names.get(m.property.name.as_str());
                    if let Some(i) = i {
                        record(ctx, node, i, false, &mut uses);
                    }
                    i
                }
                AstKind::ComputedMemberExpression(m) => {
                    if let Some(s) = helpers::string_value(&m.expression) {
                        let i = names.get(s);
                        if let Some(i) = i {
                            uses.mentioned[i] = true;
                        }
                        i
                    } else {
                        if matches!(
                            helpers::unwrap_expr(&m.object),
                            Expression::ThisExpression(_)
                        ) {
                            dynamic_classes.extend(owner_class(ctx, node));
                        }
                        None
                    }
                }
                AstKind::StringLiteral(s) => {
                    let i = names.get(s.value.as_str());
                    if let Some(i) = i {
                        uses.mentioned[i] = true;
                    }
                    i
                }
                AstKind::BindingProperty(p) => {
                    let i = p.key.static_name().and_then(|key| names.get(&key));
                    if let Some(i) = i {
                        uses.mentioned[i] = true;
                    }
                    i
                }
                AstKind::AssignmentTargetPropertyIdentifier(p) => {
                    let i = names.get(p.binding.name.as_str());
                    if let Some(i) = i {
                        uses.mentioned[i] = true;
                    }
                    i
                }
                AstKind::AssignmentTargetPropertyProperty(p) => {
                    let i = p.name.static_name().and_then(|key| names.get(&key));
                    if let Some(i) = i {
                        uses.mentioned[i] = true;
                    }
                    i
                }
                AstKind::CallExpression(call)
                    // `Object.assign(this, …)` and friends can reach any `private` member.
                    if matches!(
                        helpers::call_member_names(call),
                        Some(("Object", "assign" | "keys" | "entries" | "values"))
                    ) && call
                        .arguments
                        .first()
                        .and_then(Argument::as_expression)
                        .is_some_and(|a| {
                            matches!(helpers::unwrap_expr(a), Expression::ThisExpression(_))
                        })
                    => {
                        dynamic_classes.extend(owner_class(ctx, node));
                        None
                    }
                _ => None,
            };
            if let Some(i) = touched {
                for &k in by_name.get(i).map_or(&[][..], Vec::as_slice) {
                    if !settled[k] && never_reported(&candidates[k], i, &uses) {
                        settled[k] = true;
                        unsettled -= 1;
                    }
                }
                if unsettled == 0 {
                    break;
                }
            }
        }
        let mut reported: FxHashSet<(NodeId, &str, bool)> = FxHashSet::default();
        for candidate in &candidates {
            let Some(i) = names.get(&candidate.name) else {
                continue;
            };
            let h = usize::from(candidate.hash);
            let (read, written) = (uses.read[i][h], uses.written[i][h]);
            if candidate.brand && !written {
                continue;
            }
            let message = if candidate.hash {
                if read {
                    continue;
                }
                if candidate.kind == MemberKind::Method && written {
                    // A setter called through an assignment.
                    continue;
                }
                if written {
                    format!(
                        "Remove the private field '#{}': it is assigned but never read",
                        candidate.name
                    )
                } else {
                    format!("Remove the unused private member '#{}'", candidate.name)
                }
            } else {
                if read
                    || written
                    || uses.mentioned[i]
                    || dynamic_classes.contains(&candidate.class)
                {
                    continue;
                }
                format!("Remove the unused private member '{}'", candidate.name)
            };
            if !reported.insert((candidate.class, candidate.name.as_str(), candidate.hash)) {
                continue;
            }
            ctx.emit(candidate.span, message);
        }
    }
}

/// Whether flags already set rule out reporting `candidate` (name index `i`): the checks of the
/// report loop in `run_once` that only flags decide (flags are only ever set).
fn never_reported(candidate: &Candidate, i: usize, uses: &Uses) -> bool {
    let h = usize::from(candidate.hash);
    let (read, written) = (uses.read[i][h], uses.written[i][h]);
    if candidate.hash {
        read || (candidate.kind == MemberKind::Method && written)
    } else {
        read || written || uses.mentioned[i]
    }
}

/// Record an access to name `i`: a plain `=` target is a write, anything else a read.
fn record(ctx: &Ctx<'_>, node: &AstNode<'_>, i: usize, hash: bool, uses: &mut Uses) {
    let h = usize::from(hash);
    if uses.read[i][h] && uses.written[i][h] {
        // Both already known: this access cannot change anything.
        return;
    }
    if is_plain_write(ctx, node) {
        uses.written[i][h] = true;
    } else {
        uses.read[i][h] = true;
    }
}

/// Is the member expression only written: the left side of a plain `=` (through TypeScript
/// wrappers), or a target inside a destructuring assignment?
fn is_plain_write(ctx: &Ctx<'_>, node: &AstNode<'_>) -> bool {
    let mut child = node.span();
    for ancestor in ctx.ancestors(node) {
        match ancestor.kind() {
            AstKind::TSAsExpression(_)
            | AstKind::TSSatisfiesExpression(_)
            | AstKind::TSNonNullExpression(_)
            | AstKind::TSTypeAssertion(_) => {}
            AstKind::AssignmentExpression(a) => {
                return a.operator == AssignmentOperator::Assign && a.left.span() == child;
            }
            AstKind::ArrayAssignmentTarget(_) | AstKind::AssignmentTargetWithDefault(_) => {
                return true;
            }
            _ => return false,
        }
        child = ancestor.span();
    }
    false
}

/// Private (`#name` or TypeScript `private`) members of one class.
fn collect_candidates(id: NodeId, class: &Class<'_>, out: &mut Vec<Candidate>) {
    let component = class
        .decorators
        .iter()
        .any(|d| matches!(decorator_name(d), Some("Component" | "Directive")));
    for element in &class.body.body {
        match element {
            ClassElement::PropertyDefinition(p) => {
                if p.declare
                    || p.computed
                    || p.value.as_ref().is_some_and(is_iife)
                    || !p.decorators.is_empty()
                    || p.r#type == PropertyDefinitionType::TSAbstractPropertyDefinition
                {
                    continue;
                }
                let before = out.len();
                push_key(
                    id,
                    &p.key,
                    p.accessibility,
                    component,
                    MemberKind::Field,
                    out,
                );
                if let Some(candidate) = out.get_mut(before) {
                    candidate.brand = is_brand(p, &candidate.name);
                }
            }
            ClassElement::AccessorProperty(p) => {
                if p.computed
                    || !p.decorators.is_empty()
                    || p.r#type == AccessorPropertyType::TSAbstractAccessorProperty
                {
                    continue;
                }
                push_key(
                    id,
                    &p.key,
                    p.accessibility,
                    component,
                    MemberKind::Field,
                    out,
                );
            }
            ClassElement::MethodDefinition(m) => {
                if m.kind == MethodDefinitionKind::Constructor {
                    continue;
                }
                if m.computed
                    || !m.decorators.is_empty()
                    || m.r#type == MethodDefinitionType::TSAbstractMethodDefinition
                {
                    continue;
                }
                push_key(
                    id,
                    &m.key,
                    m.accessibility,
                    component,
                    MemberKind::Method,
                    out,
                );
            }
            _ => {}
        }
    }
}

fn push_key(
    class: NodeId,
    key: &PropertyKey<'_>,
    accessibility: Option<TSAccessibility>,
    component: bool,
    kind: MemberKind,
    out: &mut Vec<Candidate>,
) {
    match key {
        PropertyKey::PrivateIdentifier(p) => out.push(Candidate {
            name: p.name.to_string(),
            hash: true,
            kind,
            span: p.span,
            class,
            brand: false,
        }),
        PropertyKey::StaticIdentifier(k)
            if accessibility == Some(TSAccessibility::Private) && !component =>
        {
            out.push(Candidate {
                name: k.name.to_string(),
                hash: false,
                kind,
                span: k.span,
                class,
                brand: false,
            });
        }
        _ => {}
    }
}

/// `private __brand: void`, `#brand!: never`: a field that only makes the class nominally distinct
/// for TypeScript (classes with private members are compatible only with their own subclasses), so
/// nothing is meant to read it. A `_`/`$`-prefixed name ending in `brand` says so on its own; a
/// plain `brand` must also be a typed declaration without a value.
fn is_brand(p: &PropertyDefinition<'_>, name: &str) -> bool {
    let bare = name.trim_start_matches(['_', '$']);
    if bare.len() < name.len() {
        bare.to_ascii_lowercase().ends_with("brand")
    } else {
        bare.eq_ignore_ascii_case("brand") && p.value.is_none() && p.type_annotation.is_some()
    }
}

/// `(function () { … })()`: a field that exists to run its initializer once (a static-block
/// substitute).
fn is_iife(value: &Expression<'_>) -> bool {
    let Expression::CallExpression(call) = helpers::unwrap_expr(value) else {
        return false;
    };
    matches!(
        helpers::unwrap_expr(&call.callee),
        Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_)
    )
}

/// `Component` for `@Component({…})`, `@ng.Component`, `@Component`.
fn decorator_name<'a>(decorator: &Decorator<'a>) -> Option<&'a str> {
    let mut expr = &decorator.expression;
    if let Expression::CallExpression(call) = expr {
        expr = &call.callee;
    }
    match expr {
        Expression::Identifier(id) => Some(id.name.as_str()),
        Expression::StaticMemberExpression(m) => Some(m.property.name.as_str()),
        _ => None,
    }
}

/// The class whose `this` the node sees (skipping arrow functions, which inherit `this`).
fn owner_class(ctx: &Ctx<'_>, node: &AstNode<'_>) -> Option<NodeId> {
    ctx.ancestors(node)
        .find(|n| matches!(n.kind(), AstKind::Class(_)))
        .map(AstNode::id)
}
