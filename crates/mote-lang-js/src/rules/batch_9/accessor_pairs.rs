//! `js/accessor-pairs`

use crate::prelude::*;

pub struct AccessorPairs;

pub static ACCESSOR_PAIRS: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["suspicious", "confusing"],
    remediation_minutes: 5,
    description: r#"Flags a property that has a setter but no getter: a `set` accessor in a class or an object
literal without a `get` accessor of the same name (and, in a class, the same `static`-ness), and a
property descriptor given to `Object.defineProperty`, `Reflect.defineProperty` or
`Object.defineProperties` with a `set` function but no `get`.

**Why:** assigning to such a property works, but reading it always gives `undefined`, which is
rarely what the author or the callers expect (`obj.size = 3; obj.size` is `undefined`). A setter
alone on a subclass is a trap of its own: it replaces the whole inherited accessor, so the getter
of the parent class stops working for instances of the subclass.

**Fix:** add the matching getter, or replace the write-only property with a method
(`setSize(n)`) that says it only writes.

```js
class Box {
  set size(n) { this._size = n; }        // flagged: reads give undefined
}

class Box {
  get size() { return this._size; }
  set size(n) { this._size = n; }        // compliant
}
```

Not reported: getters without setters (read-only properties are normal), empty setters (stubs
that ignore writes on purpose), setters with a
decorator (framework inputs such as `@Input() set value(v)`), abstract and ambient declarations,
setters whose name is computed at run time, classes that also declare a field or method of the
same name, and object literals with a spread (which may bring the getter).
"#,
    ..RuleMeta::new(
        "js/accessor-pairs",
        "js",
        "A property with a setter should also have a getter",
    )
};

/// The name an accessor pairs on: the static name, or `#name` for a private one.
fn key_name<'a>(key: &PropertyKey<'a>) -> Option<Cow<'a, str>> {
    if let PropertyKey::PrivateIdentifier(p) = key {
        return Some(Cow::Owned(format!("#{}", p.name)));
    }
    key.static_name()
}

const MESSAGE: &str =
    "Add a getter for this setter, or replace it with a method: reading the property gives undefined";

impl JsRule for AccessorPairs {
    fn meta(&self) -> &'static RuleMeta {
        &ACCESSOR_PAIRS
    }

    fn types(&self) -> &'static [AstType] {
        &[
            AstType::Class,
            AstType::ObjectExpression,
            AstType::CallExpression,
        ]
    }

    fn callee_names(&self) -> &'static [&'static str] {
        &["defineProperty", "defineProperties"]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        match node.kind() {
            AstKind::Class(class) => check_class(node, class, ctx),
            AstKind::ObjectExpression(object) => check_object(object, ctx),
            AstKind::CallExpression(call) => check_descriptor_call(call, ctx),
            _ => {}
        }
    }
}

fn check_class<'a>(node: &AstNode<'a>, class: &'a Class<'a>, ctx: &mut Ctx<'a>) {
    // Cheap check first: any setter at all?
    let has_setter = class.body.body.iter().any(
        |e| matches!(e, ClassElement::MethodDefinition(m) if m.kind == MethodDefinitionKind::Set),
    );
    if !has_setter || class.declare || ctx.in_ambient_context(node) {
        return;
    }
    let mut found = Vec::new();
    for element in &class.body.body {
        let ClassElement::MethodDefinition(setter) = element else {
            continue;
        };
        if setter.kind != MethodDefinitionKind::Set
            || !setter.decorators.is_empty()
            || setter
                .value
                .body
                .as_ref()
                .is_none_or(|b| b.statements.is_empty())
            || setter.r#type == MethodDefinitionType::TSAbstractMethodDefinition
        {
            continue;
        }
        let Some(name) = key_name(&setter.key) else {
            continue;
        };
        let paired = class.body.body.iter().any(|other| {
            if std::ptr::eq(other, element) {
                return false;
            }
            let (is_static, key) = match other {
                ClassElement::MethodDefinition(m) => (m.r#static, &m.key),
                ClassElement::PropertyDefinition(p) => (p.r#static, &p.key),
                ClassElement::AccessorProperty(p) => (p.r#static, &p.key),
                _ => return false,
            };
            // A getter pairs with the setter; a field or method of the same name means something
            // else is going on, so stay silent too.
            is_static == setter.r#static && key_name(key).is_some_and(|n| n == name)
        });
        if !paired {
            found.push(setter.key.span());
        }
    }
    for span in found {
        ctx.emit(span, MESSAGE);
    }
}

/// `set x(v) {}`: a stub that deliberately ignores writes.
fn is_empty_function(value: &Expression<'_>) -> bool {
    matches!(value, Expression::FunctionExpression(f) if f.body.as_ref().is_none_or(|b| b.statements.is_empty()))
}

fn check_object<'a>(object: &'a ObjectExpression<'a>, ctx: &mut Ctx<'a>) {
    let has_setter = object
        .properties
        .iter()
        .any(|p| matches!(p, ObjectPropertyKind::ObjectProperty(p) if p.kind == PropertyKind::Set));
    if !has_setter {
        return;
    }
    // A spread may bring the getter (or the whole accessor) along.
    if object
        .properties
        .iter()
        .any(|p| matches!(p, ObjectPropertyKind::SpreadProperty(_)))
    {
        return;
    }
    let mut found = Vec::new();
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(setter) = property else {
            continue;
        };
        if setter.kind != PropertyKind::Set || is_empty_function(&setter.value) {
            continue;
        }
        let Some(name) = setter.key.static_name() else {
            continue;
        };
        let paired = object.properties.iter().any(|other| {
            let ObjectPropertyKind::ObjectProperty(other) = other else {
                return false;
            };
            !std::ptr::eq(&**other, &**setter) && other.key.static_name().is_some_and(|n| n == name)
        });
        if !paired {
            found.push(setter.key.span());
        }
    }
    for span in found {
        ctx.emit(span, MESSAGE);
    }
}

/// The descriptor objects of `Object.defineProperty(o, 'x', { … })`,
/// `Reflect.defineProperty(o, 'x', { … })` and `Object.defineProperties(o, { x: { … } })`.
fn descriptors<'b, 'a>(
    call: &'b CallExpression<'a>,
    object: &str,
    method: &str,
) -> Vec<&'b ObjectExpression<'a>> {
    let argument = |i: usize| call.arguments.get(i).and_then(helpers::argument_expr);
    match (object, method) {
        ("Object" | "Reflect", "defineProperty") => match argument(2).map(helpers::unwrap_expr) {
            Some(Expression::ObjectExpression(d)) => vec![&**d],
            _ => Vec::new(),
        },
        ("Object", "defineProperties") => match argument(1).map(helpers::unwrap_expr) {
            Some(Expression::ObjectExpression(map)) => map
                .properties
                .iter()
                .filter_map(|p| match p {
                    ObjectPropertyKind::ObjectProperty(p) => match helpers::unwrap_expr(&p.value) {
                        Expression::ObjectExpression(d) => Some(&**d),
                        _ => None,
                    },
                    ObjectPropertyKind::SpreadProperty(_) => None,
                })
                .collect(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

fn check_descriptor_call<'a>(call: &'a CallExpression<'a>, ctx: &mut Ctx<'a>) {
    let callee = helpers::unwrap_expr(&call.callee);
    let Some((object, method)) = helpers::static_member_names(callee) else {
        return;
    };
    if !matches!(method, "defineProperty" | "defineProperties") {
        return;
    }
    let Expression::StaticMemberExpression(member) = callee else {
        return;
    };
    if !ctx.is_global(&member.object, object) {
        return;
    }
    let mut found = Vec::new();
    for descriptor in descriptors(call, object, method) {
        let mut setter = None;
        let mut complete = false;
        for property in &descriptor.properties {
            let ObjectPropertyKind::ObjectProperty(p) = property else {
                // A spread may bring the getter.
                complete = true;
                break;
            };
            if p.computed {
                continue;
            }
            match p.key.static_name().as_deref() {
                Some("set") if p.kind == PropertyKind::Init => setter = Some(p.key.span()),
                Some("get") => complete = true,
                _ => {}
            }
        }
        if let (Some(span), false) = (setter, complete) {
            found.push(span);
        }
    }
    for span in found {
        ctx.emit(
            span,
            "Add a 'get' to this property descriptor: reading the property gives undefined",
        );
    }
}
