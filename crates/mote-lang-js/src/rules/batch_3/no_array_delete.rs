//! `js/no-array-delete`

use crate::prelude::*;

pub struct NoArrayDelete;

pub static NO_ARRAY_DELETE: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["suspicious"],
    remediation_minutes: 5,
    description: r#"Flags `delete array[index]` on a value that is declared as an array.

**Why:** `delete` removes the property but does not shift the following elements or change
`length`. The array is left with a hole: `length` still counts it, `forEach`/`map` skip it, and
indexing it yields `undefined`. Code that meant "remove this element" silently misbehaves.

**Fix:** use `array.splice(index, 1)` to remove the element in place, or `filter` / `toSpliced` to
build a new array. To keep the slot but clear it, assign `undefined` explicitly.

The rule needs evidence that the value is an array: a local variable, parameter, or class property
with an array or tuple type annotation, or one initialized with an array literal / `new Array()` /
`Array.from()` / `Array.of()` and never reassigned.

```ts
const queue: Job[] = load();
delete queue[0];              // flagged

queue.splice(0, 1);           // compliant
const cache: Record<string, Job> = {};
delete cache[key];            // compliant: not an array
```
"#,
    ..RuleMeta::new(
        "js/no-array-delete",
        "js",
        "Array elements should not be removed with 'delete'",
    )
};

impl JsRule for NoArrayDelete {
    fn meta(&self) -> &'static RuleMeta {
        &NO_ARRAY_DELETE
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::UnaryExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::UnaryExpression(unary) = node.kind() else {
            return;
        };
        if unary.operator != UnaryOperator::Delete {
            return;
        }
        let Expression::ComputedMemberExpression(member) = helpers::unwrap_expr(&unary.argument)
        else {
            return;
        };
        let object = helpers::unwrap_expr(&member.object);
        let is_array = match object {
            Expression::Identifier(id) => identifier_is_array(ctx, id),
            Expression::StaticMemberExpression(m)
                if matches!(m.object, Expression::ThisExpression(_)) =>
            {
                this_property_is_array(ctx, node, m.property.name.as_str())
            }
            _ => false,
        };
        if !is_array {
            return;
        }
        let keyword = Span::new(unary.span.start, unary.span.start + 6);
        ctx.emit(
            if ctx.text(keyword) == "delete" {
                keyword
            } else {
                unary.span
            },
            format!(
                "Use 'splice' or 'filter' to remove elements from the array '{}': 'delete' leaves a hole",
                shorten(ctx.text(object.span()))
            ),
        );
    }
}

fn shorten(text: &str) -> String {
    text.chars().take(40).collect()
}

fn identifier_is_array(ctx: &Ctx<'_>, id: &IdentifierReference<'_>) -> bool {
    let Some(symbol) = ctx.reference_symbol(id) else {
        return false;
    };
    if !ctx.scoping().symbol_redeclarations(symbol).is_empty() {
        return false;
    }
    match ctx.symbol_declaration(symbol).kind() {
        AstKind::VariableDeclarator(decl) => {
            let BindingPattern::BindingIdentifier(binding) = &decl.id else {
                return false;
            };
            if ctx.binding_symbol(binding) != Some(symbol) {
                return false;
            }
            if let Some(annotation) = &decl.type_annotation {
                return is_array_type(&annotation.type_annotation);
            }
            let never_reassigned = decl.kind == VariableDeclarationKind::Const
                || ctx.references_of(symbol).all(|r| !r.is_write());
            never_reassigned && decl.init.as_ref().is_some_and(|i| is_array_init(ctx, i))
        }
        AstKind::FormalParameter(param) => {
            matches!(param.pattern, BindingPattern::BindingIdentifier(_))
                && param
                    .type_annotation
                    .as_ref()
                    .is_some_and(|a| is_array_type(&a.type_annotation))
        }
        _ => false,
    }
}

/// `this.name` inside a method (or arrow function / field initializer) of a TypeScript class that
/// declares an instance property `name` with an array type or an array-literal initializer.
fn this_property_is_array(ctx: &Ctx<'_>, node: &AstNode<'_>, name: &str) -> bool {
    if !ctx.is_typescript() {
        return false;
    }
    let mut class_body = None;
    for ancestor in ctx.ancestors(node) {
        match ancestor.kind() {
            AstKind::ArrowFunctionExpression(_) => {}
            AstKind::Function(_) => {
                // Only a method's `this` is the instance.
                if !matches!(ctx.parent_kind(ancestor), Some(AstKind::MethodDefinition(m)) if !m.r#static)
                {
                    return false;
                }
            }
            AstKind::PropertyDefinition(p) if p.r#static => return false,
            AstKind::StaticBlock(_) => return false,
            AstKind::ClassBody(body) => {
                class_body = Some(body);
                break;
            }
            _ => {}
        }
    }
    let Some(body) = class_body else {
        return false;
    };
    body.body.iter().any(|element| match element {
        ClassElement::PropertyDefinition(p) => {
            !p.r#static
                && !p.computed
                && p.key.is_specific_static_name(name)
                && match &p.type_annotation {
                    Some(annotation) => is_array_type(&annotation.type_annotation),
                    None => p
                        .value
                        .as_ref()
                        .is_some_and(|v| matches!(v, Expression::ArrayExpression(_))),
                }
        }
        _ => false,
    })
}

/// `T[]`, `readonly T[]`, `[A, B]`, `Array<T>`, `ReadonlyArray<T>`.
fn is_array_type(ty: &TSType<'_>) -> bool {
    match ty {
        TSType::TSArrayType(_) | TSType::TSTupleType(_) => true,
        TSType::TSTypeOperatorType(op) => is_array_type(&op.type_annotation),
        TSType::TSParenthesizedType(p) => is_array_type(&p.type_annotation),
        TSType::TSTypeReference(r) => matches!(&r.type_name,
            TSTypeName::IdentifierReference(id) if id.name == "Array" || id.name == "ReadonlyArray"),
        _ => false,
    }
}

/// `[…]`, `new Array(…)`, `Array(…)`, `Array.from(…)`, `Array.of(…)`.
fn is_array_init(ctx: &Ctx<'_>, init: &Expression<'_>) -> bool {
    match helpers::unwrap_expr(init) {
        Expression::ArrayExpression(_) => true,
        Expression::NewExpression(new) => ctx.is_global(&new.callee, "Array"),
        Expression::CallExpression(call) => match helpers::unwrap_expr(&call.callee) {
            callee @ Expression::Identifier(_) => ctx.is_global(callee, "Array"),
            Expression::StaticMemberExpression(m) => {
                matches!(m.property.name.as_str(), "from" | "of")
                    && ctx.is_global(&m.object, "Array")
            }
            _ => false,
        },
        _ => false,
    }
}
