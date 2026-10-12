//! `js/mutable-public-static-field`

use crate::prelude::*;

pub struct MutablePublicStaticField;

pub static MUTABLE_PUBLIC_STATIC_FIELD: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["design", "encapsulation"],
    remediation_minutes: 10,
    params: &[ParamSpec {
        name: "allow",
        default: ParamDefault::Str(""),
        description: "Comma-separated names of static fields to accept, for conventions a framework reads from the class (e.g. `defaultOption,dependencies`).",
    }],
    description: r#"Flags public `static` fields of exported TypeScript classes that are not `readonly` and hold a
mutable container: an array or object literal, or a new `Map`, `Set`, `WeakMap`, `WeakSet`,
`Array` or `Object`.

**Why:** such a field is global mutable state. Any module that imports the class can replace the
container or change its contents at any time, without validation, and every other user of the
class sees the change. Bugs caused by it are hard to trace, and tests influence each other
through it.

**Fix:** make the field `private` (or `#private`) and expose what callers need through methods;
for a constant table, make it `readonly` with a read-only type (`readonly string[]`,
`ReadonlyMap`, `as const`) or freeze it.

```ts
export class Registry {
  static handlers = new Map<string, Handler>();          // flagged
  static defaults = { retries: 3 };                      // flagged
}

export class Registry {
  private static handlers = new Map<string, Handler>();  // compliant
  static readonly DEFAULTS = Object.freeze({ retries: 3 }); // compliant
  static register(name: string, h: Handler) { Registry.handlers.set(name, h); }
}
```

Not reported: `readonly`, `private`, `protected` and `#private` fields, fields with a decorator or
`declare`, fields typed or asserted as read-only (`as const`), fields holding functions or other
values, conventional framework statics (`defaultProps`, `propTypes`, `contextTypes`,
`childContextTypes`, `styles`, `properties`, `observedAttributes`, `$inject`) and the names listed
in the `allow` parameter, classes that the module does not export, plain JavaScript files (which
have no `readonly`), and test files. Library-specific conventions (a chart library's
`defaultOption`, say) are reported by default: other modules can and do replace or change such a
shared default; list them in `allow` where a project accepts the convention.
"#,
    ..RuleMeta::new(
        "js/mutable-public-static-field",
        "js",
        "Public static fields should not hold shared mutable containers",
    )
};

/// Framework statics whose shape the framework dictates.
const FRAMEWORK_STATICS: &[&str] = &[
    "defaultProps",
    "propTypes",
    "contextTypes",
    "childContextTypes",
    "styles",
    "properties",
    "observedAttributes",
    "$inject",
];

/// What mutable container does this initializer create, if any?
fn container(ctx: &Ctx<'_>, init: &Expression<'_>) -> Option<&'static str> {
    // `[...] as const` is read-only.
    if let Expression::TSAsExpression(a) = init {
        if let TSType::TSTypeReference(r) = &a.type_annotation {
            if matches!(&r.type_name, TSTypeName::IdentifierReference(id) if id.name == "const") {
                return None;
            }
        }
        if is_readonly_type(&a.type_annotation) {
            return None;
        }
    }
    match helpers::unwrap_expr(init) {
        Expression::ArrayExpression(_) => Some("array"),
        Expression::ObjectExpression(_) => Some("object"),
        Expression::NewExpression(new) => {
            let Expression::Identifier(id) = helpers::unwrap_expr(&new.callee) else {
                return None;
            };
            let kind = match id.name.as_str() {
                "Map" | "WeakMap" => "map",
                "Set" | "WeakSet" => "set",
                "Array" => "array",
                "Object" => "object",
                _ => return None,
            };
            ctx.is_global_reference(id).then_some(kind)
        }
        _ => None,
    }
}

/// `readonly T[]`, `ReadonlyArray<T>`, `ReadonlyMap`, `ReadonlySet`, `Readonly<T>`.
fn is_readonly_type(ty: &TSType<'_>) -> bool {
    match ty {
        TSType::TSTypeOperatorType(op) => op.operator == TSTypeOperatorOperator::Readonly,
        TSType::TSTypeReference(r) => matches!(
            &r.type_name,
            TSTypeName::IdentifierReference(id)
                if matches!(id.name.as_str(), "ReadonlyArray" | "ReadonlyMap" | "ReadonlySet" | "Readonly")
        ),
        _ => false,
    }
}

/// Is the class visible to other modules: exported, or declared in a script (a global)?
fn is_exported(ctx: &Ctx<'_>, node: &AstNode<'_>, class: &Class<'_>) -> bool {
    match ctx.parent_kind(node) {
        Some(AstKind::ExportNamedDeclaration(_) | AstKind::ExportDefaultDeclaration(_)) => {
            return true;
        }
        Some(AstKind::Program(_)) => {}
        _ => return false,
    }
    if !ctx.is_module() {
        return true;
    }
    let Some(symbol) = class.id.as_ref().and_then(|id| ctx.binding_symbol(id)) else {
        return false;
    };
    ctx.references_of(symbol).any(|r| {
        matches!(
            ctx.parent_kind(r.node_id()),
            Some(AstKind::ExportSpecifier(_) | AstKind::ExportDefaultDeclaration(_))
        )
    })
}

impl JsRule for MutablePublicStaticField {
    fn meta(&self) -> &'static RuleMeta {
        &MUTABLE_PUBLIC_STATIC_FIELD
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::Class]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.is_typescript() && !ctx.is_test()
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::Class(class) = node.kind() else {
            return;
        };
        let mut found = Vec::new();
        for element in &class.body.body {
            let ClassElement::PropertyDefinition(field) = element else {
                continue;
            };
            if !field.r#static
                || field.readonly
                || field.declare
                || !field.decorators.is_empty()
                || matches!(
                    field.accessibility,
                    Some(TSAccessibility::Private | TSAccessibility::Protected)
                )
                || matches!(field.key, PropertyKey::PrivateIdentifier(_))
            {
                continue;
            }
            let Some(init) = &field.value else {
                continue;
            };
            if field
                .type_annotation
                .as_ref()
                .is_some_and(|t| is_readonly_type(&t.type_annotation))
            {
                continue;
            }
            let Some(kind) = container(ctx, init) else {
                continue;
            };
            let Some(name) = field.key.static_name() else {
                continue;
            };
            if FRAMEWORK_STATICS.contains(&name.as_ref())
                || ctx
                    .param_list("allow")
                    .any(|allowed| allowed == name.as_ref())
            {
                continue;
            }
            found.push((field.key.span(), name, kind));
        }
        if found.is_empty() || class.declare || !is_exported(ctx, node, class) {
            return;
        }
        for (span, name, kind) in found {
            ctx.emit(
                span,
                format!(
                    "Make the public static field '{name}' private, or readonly with a read-only type: any module can replace or change this shared {kind}"
                ),
            );
        }
    }
}
