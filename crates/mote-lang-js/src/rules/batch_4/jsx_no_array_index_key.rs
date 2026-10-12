//! `js/jsx-no-array-index-key`

use super::util;
use crate::prelude::*;

pub struct JsxNoArrayIndexKey;

pub static JSX_NO_ARRAY_INDEX_KEY: RuleMeta = RuleMeta {
    tags: &["react", "jsx"],
    remediation_minutes: 10,
    description: r#"Flags JSX `key` props computed only from the index parameter of an array callback
(`items.map((item, i) => <Row key={i} />)`), including `String(i)`, `i.toString()`, `` `${i}` ``,
and `'row-' + i`.

**Why:** React uses `key` to match list children between renders. An index is not tied to the item:
when items are inserted, removed, filtered, or reordered, each index now points at a different item,
so React re-uses the wrong component instances. Local state, focus, and uncontrolled input values
end up on the wrong rows, and more of the list is re-rendered than needed.

**Fix:** use a value that identifies the item itself, such as an id from the data
(`key={item.id}`). An index is acceptable only for static lists that never change order; add a
suppression comment there to document it.

```jsx
todos.map((todo, index) => <Todo key={index} {...todo} />);   // flagged
todos.map((todo) => <Todo key={todo.id} {...todo} />);        // compliant
```

Only React/Preact files are checked. Keys that combine the index with other values
(`` `${row.id}-${i}` ``) and lists built with `Array.from({ length }, …)` are not flagged.
"#,
    ..RuleMeta::new(
        "js/jsx-no-array-index-key",
        "js",
        "Array indexes should not be used as JSX keys",
    )
};

/// Array methods whose callback receives the index as its second parameter.
const INDEX_SECOND: &[&str] = &[
    "map",
    "forEach",
    "filter",
    "flatMap",
    "some",
    "every",
    "find",
    "findIndex",
    "findLast",
    "findLastIndex",
];

impl JsRule for JsxNoArrayIndexKey {
    fn meta(&self) -> &'static RuleMeta {
        &JSX_NO_ARRAY_INDEX_KEY
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::JSXAttribute]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        util::has_jsx(ctx) && util::is_react_file(ctx)
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::JSXAttribute(attr) = node.kind() else {
            return;
        };
        let JSXAttributeName::Identifier(name) = &attr.name else {
            return;
        };
        if name.name != "key" {
            return;
        }
        let Some(JSXAttributeValue::ExpressionContainer(container)) = &attr.value else {
            return;
        };
        let Some(expr) = container.expression.as_expression() else {
            return;
        };
        let mut index: Option<&IdentifierReference<'a>> = None;
        if !index_only(ctx, expr, &mut index) {
            return;
        }
        let Some(id) = index else {
            return;
        };
        let Some(symbol) = ctx.reference_symbol(id) else {
            return;
        };
        if !is_index_parameter(ctx, symbol) {
            return;
        }
        ctx.emit(
            expr.span(),
            format!(
                "Use a stable id from the item as the 'key' instead of the array index '{}'",
                id.name
            ),
        );
    }
}

/// Is the expression built only from literals and one identifier (the candidate index), through
/// `String(i)`, `i.toString()`, template literals, and `+`? The identifier is stored in `index`;
/// two different identifiers make the key more than an index.
fn index_only<'a>(
    ctx: &Ctx<'a>,
    expr: &'a Expression<'a>,
    index: &mut Option<&'a IdentifierReference<'a>>,
) -> bool {
    match helpers::unwrap_expr(expr) {
        Expression::Identifier(id) => match index {
            Some(seen) => seen.name == id.name,
            None => {
                *index = Some(id);
                true
            }
        },
        Expression::StringLiteral(_) | Expression::NumericLiteral(_) => true,
        Expression::TemplateLiteral(t) => {
            !t.expressions.is_empty() && t.expressions.iter().all(|e| index_only(ctx, e, index))
        }
        Expression::BinaryExpression(b) if b.operator == BinaryOperator::Addition => {
            index_only(ctx, &b.left, index) && index_only(ctx, &b.right, index)
        }
        Expression::CallExpression(call) => match helpers::unwrap_expr(&call.callee) {
            // `String(i)`
            Expression::Identifier(callee)
                if callee.name == "String"
                    && ctx.is_global_reference(callee)
                    && call.arguments.len() == 1 =>
            {
                call.arguments
                    .first()
                    .and_then(Argument::as_expression)
                    .is_some_and(|arg| {
                        matches!(helpers::unwrap_expr(arg), Expression::Identifier(_))
                            && index_only(ctx, arg, index)
                    })
            }
            // `i.toString()`
            Expression::StaticMemberExpression(m)
                if m.property.name == "toString" && call.arguments.is_empty() =>
            {
                matches!(helpers::unwrap_expr(&m.object), Expression::Identifier(_))
                    && index_only(ctx, &m.object, index)
            }
            _ => false,
        },
        _ => false,
    }
}

/// The symbol is the index parameter of a callback passed to an array iteration method
/// (`list.map((item, i) => …)`) or to `Children.map` / `Children.forEach`.
fn is_index_parameter(ctx: &Ctx<'_>, symbol: SymbolId) -> bool {
    let decl = ctx.symbol_declaration(symbol);
    let AstKind::FormalParameter(param) = decl.kind() else {
        return false;
    };
    let Some(params_node) = ctx.parent(decl) else {
        return false;
    };
    let AstKind::FormalParameters(params) = params_node.kind() else {
        return false;
    };
    let Some(position) = params.items.iter().position(|p| p.span == param.span) else {
        return false;
    };
    let Some(function) = ctx.parent(params_node) else {
        return false;
    };
    if !matches!(
        function.kind(),
        AstKind::ArrowFunctionExpression(_) | AstKind::Function(_)
    ) {
        return false;
    }
    let function_span = function.span();
    let Some(call_node) = ctx.outer_parent(function) else {
        return false;
    };
    let AstKind::CallExpression(call) = call_node.kind() else {
        return false;
    };
    let callback_at = call
        .arguments
        .iter()
        .position(|a| a.span() == function_span);
    let (object, method) = match helpers::unwrap_expr(&call.callee) {
        Expression::StaticMemberExpression(m) => {
            (helpers::unwrap_expr(&m.object), m.property.name.as_str())
        }
        Expression::ChainExpression(c) => match &c.expression {
            ChainElement::StaticMemberExpression(m) => {
                (helpers::unwrap_expr(&m.object), m.property.name.as_str())
            }
            _ => return false,
        },
        _ => return false,
    };
    let children_api = match object {
        Expression::Identifier(id) => id.name == "Children",
        Expression::StaticMemberExpression(m) => m.property.name == "Children",
        _ => false,
    };
    if children_api && (method == "map" || method == "forEach") {
        return callback_at == Some(1) && position == 1;
    }
    if callback_at != Some(0) {
        return false;
    }
    if INDEX_SECOND.contains(&method) {
        position == 1
    } else if method == "reduce" || method == "reduceRight" {
        position == 2
    } else {
        false
    }
}
