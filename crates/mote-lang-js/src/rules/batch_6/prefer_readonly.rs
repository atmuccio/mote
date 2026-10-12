//! `js/prefer-readonly`

use crate::prelude::*;

pub struct PreferReadonly;

pub static PREFER_READONLY: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["typescript", "immutability", "conventions"],
    remediation_minutes: 1,
    description: r#"Flags private class members (TypeScript `private` fields, parameter properties, and `#name`
fields) that are never assigned after construction and could be declared `readonly`.

**Why:** `readonly` tells readers that a member keeps the value it got at construction, and lets
the compiler reject a later assignment that would break that assumption. A private member is only
visible inside its class, so whether it is ever reassigned can be decided from the class alone.

**Fix:** add `readonly` to the declaration.

```ts
class Cache {
  private store = new Map<string, number>();     // flagged: never reassigned
  constructor(private http: HttpClient) {}       // flagged
  private hits = 0;                              // compliant: incremented in get()
  get(key: string) { this.hits++; return this.store.get(key); }
}

class Cache {
  private readonly store = new Map<string, number>(); // compliant
  constructor(private readonly http: HttpClient) {}
}
```

Assignments in the constructor itself count as initialization; an assignment anywhere else —
another method, a callback or arrow function created in the constructor, a static block, or a
write through another instance of the class — makes the member mutable. Not reported: static
members, decorated members and parameters (frameworks may assign them), accessors, and classes
that write to `this` through computed keys or `Object.assign(this, …)`. Off by default: the rule
states a style of declaring immutability rather than finding a defect.
"#,
    ..RuleMeta::new(
        "js/prefer-readonly",
        "js",
        "Private members that are never reassigned should be readonly",
    )
};

struct Candidate {
    name: String,
    private_name: bool,
    span: Span,
    class: NodeId,
}

/// Candidates of the file's classes, in source order.
#[derive(Default)]
struct Candidates {
    list: Vec<Candidate>,
}

impl JsRule for PreferReadonly {
    fn meta(&self) -> &'static RuleMeta {
        &PREFER_READONLY
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.is_typescript() && ctx.file_contains(AstType::Class)
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
        // Names written outside a constructor of the declaring class, and classes that write to
        // `this` dynamically.
        // `[plain name, #name]` per name of `names`.
        let mut mutated: Vec<[bool; 2]> = vec![[false; 2]; names.len()];
        // The `mutated` slots some candidate reads, and how many of them are still unset: once
        // none is, no candidate can be reported and the rest of the file need not be read.
        let mut needed: Vec<[bool; 2]> = vec![[false; 2]; names.len()];
        let mut pending = 0usize;
        for c in &candidates {
            if let Some(i) = names.get(&c.name) {
                let slot = &mut needed[i][usize::from(c.private_name)];
                pending += usize::from(!*slot);
                *slot = true;
            }
        }
        let mut dynamic_classes: FxHashSet<NodeId> = FxHashSet::default();
        for node in ctx.nodes().iter() {
            if pending == 0 {
                break;
            }
            let (name, private_name, object) = match node.kind() {
                AstKind::StaticMemberExpression(m) => (m.property.name.as_str(), false, &m.object),
                AstKind::PrivateFieldExpression(m) => (m.field.name.as_str(), true, &m.object),
                AstKind::ComputedMemberExpression(m) => {
                    // Only a write to a listed string key or through `this[…]` matters; the
                    // cheap checks come before the ancestor walk.
                    let listed = match &m.expression {
                        Expression::StringLiteral(s) => names.get(s.value.as_str()),
                        _ => None,
                    };
                    match listed {
                        Some(i) => {
                            if !mutated[i][0] && is_write_target(ctx, node) {
                                mutated[i][0] = true;
                                pending -= usize::from(needed[i][0]);
                            }
                        }
                        None => {
                            if matches!(
                                helpers::unwrap_expr(&m.object),
                                Expression::ThisExpression(_)
                            ) && is_write_target(ctx, node)
                            {
                                dynamic_classes.extend(owner_class(ctx, node));
                            }
                        }
                    }
                    continue;
                }
                AstKind::CallExpression(call) => {
                    // `Object.assign(this, …)` can set any member.
                    if helpers::call_member_names(call) == Some(("Object", "assign"))
                        && call
                            .arguments
                            .first()
                            .and_then(Argument::as_expression)
                            .is_some_and(|a| {
                                matches!(helpers::unwrap_expr(a), Expression::ThisExpression(_))
                            })
                    {
                        dynamic_classes.extend(owner_class(ctx, node));
                    }
                    continue;
                }
                _ => continue,
            };
            let Some(i) = names.get(name) else {
                continue;
            };
            let p = usize::from(private_name);
            // Already known to be mutated: this access cannot change anything.
            if mutated[i][p] || !is_write_target(ctx, node) {
                continue;
            }
            if matches!(helpers::unwrap_expr(object), Expression::ThisExpression(_))
                && in_own_constructor(ctx, node)
            {
                continue;
            }
            mutated[i][p] = true;
            pending -= usize::from(needed[i][p]);
        }
        for candidate in &candidates {
            let mutated = names
                .get(&candidate.name)
                .is_some_and(|i| mutated[i][usize::from(candidate.private_name)]);
            if mutated || dynamic_classes.contains(&candidate.class) {
                continue;
            }
            let shown = if candidate.private_name {
                format!("#{}", candidate.name)
            } else {
                candidate.name.clone()
            };
            ctx.emit(
                candidate.span,
                format!("Mark '{shown}' as readonly: it is never reassigned after construction"),
            );
        }
    }
}

/// Private, non-static, non-readonly, undecorated fields and constructor parameter properties.
fn collect_candidates(id: NodeId, class: &Class<'_>, out: &mut Vec<Candidate>) {
    let mut accessors: FxHashSet<&str> = FxHashSet::default();
    for element in &class.body.body {
        match element {
            ClassElement::MethodDefinition(m) if m.kind != MethodDefinitionKind::Constructor => {
                if let Some(name) = key_name(&m.key) {
                    accessors.insert(name.0);
                }
            }
            ClassElement::AccessorProperty(p) => {
                if let Some(name) = key_name(&p.key) {
                    accessors.insert(name.0);
                }
            }
            _ => {}
        }
    }
    for element in &class.body.body {
        match element {
            ClassElement::PropertyDefinition(p) => {
                if p.r#static
                    || p.readonly
                    || p.declare
                    || p.computed
                    || !p.decorators.is_empty()
                    || p.r#type == PropertyDefinitionType::TSAbstractPropertyDefinition
                {
                    continue;
                }
                let Some((name, private_name)) = key_name(&p.key) else {
                    continue;
                };
                if !(private_name || p.accessibility == Some(TSAccessibility::Private))
                    || accessors.contains(name)
                {
                    continue;
                }
                out.push(Candidate {
                    name: name.to_string(),
                    private_name,
                    span: p.key.span(),
                    class: id,
                });
            }
            ClassElement::MethodDefinition(m) if m.kind == MethodDefinitionKind::Constructor => {
                for param in &m.value.params.items {
                    if param.accessibility != Some(TSAccessibility::Private)
                        || param.readonly
                        || !param.decorators.is_empty()
                    {
                        continue;
                    }
                    let binding = match &param.pattern {
                        BindingPattern::BindingIdentifier(b) => b,
                        BindingPattern::AssignmentPattern(a) => match &a.left {
                            BindingPattern::BindingIdentifier(b) => b,
                            _ => continue,
                        },
                        _ => continue,
                    };
                    if accessors.contains(binding.name.as_str()) {
                        continue;
                    }
                    out.push(Candidate {
                        name: binding.name.to_string(),
                        private_name: false,
                        span: binding.span,
                        class: id,
                    });
                }
            }
            _ => {}
        }
    }
}

/// `name` or `#name` of a class element key (`true` for `#name`).
fn key_name<'a>(key: &'a PropertyKey<'a>) -> Option<(&'a str, bool)> {
    match key {
        PropertyKey::PrivateIdentifier(p) => Some((p.name.as_str(), true)),
        PropertyKey::StaticIdentifier(id) => Some((id.name.as_str(), false)),
        PropertyKey::StringLiteral(s) => Some((s.value.as_str(), false)),
        _ => None,
    }
}

/// Is the member expression written: the target of an assignment (also inside a destructuring
/// pattern or through a TypeScript wrapper), of `++`/`--`, of a `for…in`/`for…of` head, or of
/// `delete`?
fn is_write_target(ctx: &Ctx<'_>, node: &AstNode<'_>) -> bool {
    let mut child = node.span();
    for ancestor in ctx.ancestors(node) {
        match ancestor.kind() {
            AstKind::TSAsExpression(_)
            | AstKind::TSSatisfiesExpression(_)
            | AstKind::TSNonNullExpression(_)
            | AstKind::TSTypeAssertion(_)
            | AstKind::ParenthesizedExpression(_) => {}
            AstKind::AssignmentExpression(a) => return a.left.span() == child,
            AstKind::UpdateExpression(_)
            | AstKind::ArrayAssignmentTarget(_)
            | AstKind::ObjectAssignmentTarget(_)
            | AstKind::AssignmentTargetWithDefault(_)
            | AstKind::AssignmentTargetPropertyProperty(_)
            | AstKind::AssignmentTargetRest(_) => return true,
            AstKind::ForInStatement(f) => return f.left.span() == child,
            AstKind::ForOfStatement(f) => return f.left.span() == child,
            AstKind::UnaryExpression(u) => return u.operator == UnaryOperator::Delete,
            _ => return false,
        }
        child = ancestor.span();
    }
    false
}

/// Is the node directly in the constructor of its own class (not in a nested function)?
fn in_own_constructor(ctx: &Ctx<'_>, node: &AstNode<'_>) -> bool {
    let Some(function) = ctx.ancestors(node).find(|n| {
        matches!(
            n.kind(),
            AstKind::Function(_)
                | AstKind::ArrowFunctionExpression(_)
                | AstKind::StaticBlock(_)
                | AstKind::PropertyDefinition(_)
        )
    }) else {
        return false;
    };
    matches!(ctx.parent_kind(function), Some(AstKind::MethodDefinition(m))
        if m.kind == MethodDefinitionKind::Constructor)
}

/// The class whose `this` the node sees.
fn owner_class(ctx: &Ctx<'_>, node: &AstNode<'_>) -> Option<NodeId> {
    ctx.ancestors(node)
        .find(|n| matches!(n.kind(), AstKind::Class(_)))
        .map(AstNode::id)
}
