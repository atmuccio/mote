//! `js/no-prototype-builtins`

use crate::prelude::*;

pub struct NoPrototypeBuiltins;

pub static NO_PROTOTYPE_BUILTINS: RuleMeta = RuleMeta {
    severity: Severity::Medium,
    tags: &["suspicious", "objects"],
    remediation_minutes: 2,
    description: r#"Flags calls of `hasOwnProperty`, `isPrototypeOf`, and `propertyIsEnumerable` made
directly on an object: `obj.hasOwnProperty('id')`.

**Why:** these methods come from `Object.prototype`, so calling them through the object assumes
the object inherits them unchanged. Objects created with `Object.create(null)` have no prototype
(the call throws `TypeError: obj.hasOwnProperty is not a function`), and any object parsed from
JSON or built from user input can carry an own property named `hasOwnProperty` that shadows the
built-in — a crash at best, a security hole at worst.

**Fix:** use `Object.hasOwn(obj, key)` (ES2022), or call the prototype method explicitly with
`Object.prototype.hasOwnProperty.call(obj, key)`.

```js
if (config.hasOwnProperty('port')) {}                    // flagged
if (Object.hasOwn(config, 'port')) {}                    // compliant
if (Object.prototype.hasOwnProperty.call(config, 'port')) {} // compliant
```
"#,
    ..RuleMeta::new(
        "js/no-prototype-builtins",
        "js",
        "Object.prototype methods should not be called directly on objects",
    )
};

impl JsRule for NoPrototypeBuiltins {
    fn meta(&self) -> &'static RuleMeta {
        &NO_PROTOTYPE_BUILTINS
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CallExpression]
    }

    fn callee_names(&self) -> &'static [&'static str] {
        &["hasOwnProperty", "isPrototypeOf", "propertyIsEnumerable"]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::CallExpression(call) = node.kind() else {
            return;
        };
        let (object, name, name_span) = match helpers::unwrap_expr(&call.callee) {
            Expression::StaticMemberExpression(m) => {
                (&m.object, m.property.name.as_str(), m.property.span)
            }
            Expression::ComputedMemberExpression(m) => match &m.expression {
                Expression::StringLiteral(s) => (&m.object, s.value.as_str(), s.span),
                _ => return,
            },
            // `obj?.hasOwnProperty(key)`
            Expression::ChainExpression(chain) => match &chain.expression {
                ChainElement::CallExpression(_) => return,
                ChainElement::StaticMemberExpression(m) => {
                    (&m.object, m.property.name.as_str(), m.property.span)
                }
                _ => return,
            },
            _ => return,
        };
        if !matches!(
            name,
            "hasOwnProperty" | "isPrototypeOf" | "propertyIsEnumerable"
        ) {
            return;
        }
        if is_prototype_holder(ctx, helpers::unwrap_expr(object)) {
            return;
        }
        let fix = if name == "hasOwnProperty" {
            "Object.hasOwn(obj, key)".to_string()
        } else {
            format!("Object.prototype.{name}.call(obj, …)")
        };
        ctx.emit(
            name_span,
            format!("Call '{name}' through {fix} instead of on the object itself"),
        );
    }
}

/// Receivers on which the call is not the risky "method looked up on arbitrary data" pattern:
/// `Object.prototype.hasOwnProperty(…)`, `Foo.prototype.isPrototypeOf(x)` (checking the
/// prototype chain of a class is what `isPrototypeOf` is for), and `super`.
fn is_prototype_holder(ctx: &Ctx<'_>, object: &Expression<'_>) -> bool {
    match object {
        Expression::Super(_) => true,
        Expression::StaticMemberExpression(m) => m.property.name == "prototype",
        Expression::Identifier(id) => {
            // `Object.isPrototypeOf(…)` style calls on the global constructors.
            ctx.is_global_reference(id) && matches!(id.name.as_str(), "Object" | "Reflect")
        }
        _ => false,
    }
}
