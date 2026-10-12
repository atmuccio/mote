//! `js/jsx-key`

use crate::prelude::*;

pub struct JsxKey;

pub static JSX_KEY: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["react", "jsx"],
    remediation_minutes: 5,
    description: r#"Flags JSX elements created in a list without a `key` prop: elements of an array
literal made only of JSX, and elements returned from the callback of `.map()`, `.flatMap()`, or
`Array.from(items, callback)`.

**Why:** React identifies the children of a list by their `key`. Without it, React falls back to
the position in the list, so inserting, removing, or reordering items re-uses the wrong
component instances: state and focus jump to other rows, inputs show stale values, and more DOM
is re-created than necessary. React also logs a warning for every such list.

**Fix:** give the outermost element returned for each item a `key` that is stable and unique among
its siblings — usually an id from the data (`key={user.id}`). A fragment in a list needs the long
form `<Fragment key={…}>`, because `<>…</>` cannot take a key.

Only files that show they use React or Preact are checked — an import from a React-family package
(`react`, `preact`, `react-dom`, `next/…`, `mobx-react-lite`, …), a global `React`, or a JSX
pragma naming React — because other JSX runtimes (Solid, prompt renderers, server-side HTML
templating) have no keys. Not flagged either: elements with a spread attribute (`{...props}` may
provide the key), elements inside `React.Children.toArray(…)` (which assigns keys itself), and
arrays that also contain non-JSX values (typically tuples such as `[value, <Dialog />]` returned
from a hook).

```jsx
<ul>{users.map((user) => <li>{user.name}</li>)}</ul>             // flagged

<ul>{users.map((user) => <li key={user.id}>{user.name}</li>)}</ul> // compliant
```
"#,
    ..RuleMeta::new(
        "js/jsx-key",
        "js",
        "Elements rendered in a list should have a 'key' prop",
    )
};

#[derive(Clone, Copy)]
enum Origin {
    Array,
    Iterator,
}

impl JsRule for JsxKey {
    fn meta(&self) -> &'static RuleMeta {
        &JSX_KEY
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::ArrayExpression, AstType::CallExpression]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        super::util::has_jsx(ctx) && super::util::is_react_file(ctx)
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        match node.kind() {
            AstKind::ArrayExpression(array) => {
                if array.elements.is_empty() {
                    return;
                }
                let all_jsx = array.elements.iter().all(|element| {
                    element.as_expression().is_some_and(|e| {
                        matches!(
                            helpers::unwrap_expr(e),
                            Expression::JSXElement(_) | Expression::JSXFragment(_)
                        )
                    })
                });
                if !all_jsx {
                    return;
                }
                let mut missing = Vec::new();
                for element in &array.elements {
                    if let Some(e) = element.as_expression() {
                        collect_missing(helpers::unwrap_expr(e), &mut missing);
                    }
                }
                report(ctx, node, &missing, Origin::Array);
            }
            AstKind::CallExpression(call) => {
                let Some(callback) = list_callback(ctx, call) else {
                    return;
                };
                let mut missing = Vec::new();
                match callback {
                    Expression::ArrowFunctionExpression(arrow) if arrow.expression => {
                        if let Some(Statement::ExpressionStatement(s)) =
                            arrow.body.statements.first()
                        {
                            collect_returned(&s.expression, &mut missing);
                        }
                    }
                    Expression::ArrowFunctionExpression(arrow) => {
                        super::util::for_each_return(&arrow.body.statements, &mut |r| {
                            if let Some(arg) = &r.argument {
                                collect_returned(arg, &mut missing);
                            }
                        });
                    }
                    Expression::FunctionExpression(function) => {
                        if let Some(body) = &function.body {
                            super::util::for_each_return(&body.statements, &mut |r| {
                                if let Some(arg) = &r.argument {
                                    collect_returned(arg, &mut missing);
                                }
                            });
                        }
                    }
                    _ => {}
                }
                report(ctx, node, &missing, Origin::Iterator);
            }
            _ => {}
        }
    }
}

/// A JSX value lacking a key: `(span to report, is_fragment)`.
type Missing = (Span, bool);

/// The inline callback of `items.map(cb)`, `items.flatMap(cb)`, or `Array.from(items, cb)`.
fn list_callback<'a>(ctx: &Ctx<'a>, call: &'a CallExpression<'a>) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(member) = helpers::unwrap_expr(&call.callee) else {
        return None;
    };
    let index = match member.property.name.as_str() {
        "map" | "flatMap" => 0,
        "from" if ctx.is_global(&member.object, "Array") => 1,
        _ => return None,
    };
    let callback = helpers::unwrap_expr(call.arguments.get(index)?.as_expression()?);
    matches!(
        callback,
        Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_)
    )
    .then_some(callback)
}

/// A value returned for each item: the element itself, or both branches of `c ? <A/> : <B/>`, or
/// the right side of `c && <A/>`.
fn collect_returned(expr: &Expression<'_>, out: &mut Vec<Missing>) {
    match helpers::unwrap_expr(expr) {
        e @ (Expression::JSXElement(_) | Expression::JSXFragment(_)) => collect_missing(e, out),
        Expression::ConditionalExpression(c) => {
            collect_returned(&c.consequent, out);
            collect_returned(&c.alternate, out);
        }
        Expression::LogicalExpression(l) => collect_returned(&l.right, out),
        _ => {}
    }
}

fn collect_missing(expr: &Expression<'_>, out: &mut Vec<Missing>) {
    match expr {
        Expression::JSXElement(element) => {
            let opening = &element.opening_element;
            let has_key_or_spread = opening.attributes.iter().any(|item| match item {
                JSXAttributeItem::SpreadAttribute(_) => true,
                JSXAttributeItem::Attribute(attr) => {
                    matches!(&attr.name, JSXAttributeName::Identifier(id) if id.name == "key")
                }
            });
            if !has_key_or_spread {
                out.push((opening.name.span(), false));
            }
        }
        Expression::JSXFragment(fragment) => out.push((fragment.opening_fragment.span, true)),
        _ => {}
    }
}

fn report(ctx: &mut Ctx<'_>, node: &AstNode<'_>, missing: &[Missing], origin: Origin) {
    if missing.is_empty() || inside_children_to_array(ctx, node) {
        return;
    }
    let place = match origin {
        Origin::Array => "an array",
        Origin::Iterator => "a list",
    };
    for &(span, is_fragment) in missing {
        let message = if is_fragment {
            format!(
                "Replace this '<>' with '<Fragment key={{…}}>': elements rendered in {place} need a key"
            )
        } else {
            format!("Add a 'key' prop to this element: it is rendered in {place}")
        };
        ctx.emit(span, message);
    }
}

/// Is the node an argument (directly or through `.map`) of `Children.toArray(…)`, which assigns
/// keys by itself?
fn inside_children_to_array(ctx: &Ctx<'_>, node: &AstNode<'_>) -> bool {
    for ancestor in ctx.ancestors(node) {
        match ancestor.kind() {
            AstKind::CallExpression(call) => {
                let callee = helpers::unwrap_expr(&call.callee);
                if let Expression::StaticMemberExpression(m) = callee {
                    if m.property.name == "toArray" {
                        let object = helpers::unwrap_expr(&m.object);
                        let is_children = match object {
                            Expression::Identifier(id) => id.name == "Children",
                            Expression::StaticMemberExpression(inner) => {
                                inner.property.name == "Children"
                            }
                            _ => false,
                        };
                        if is_children {
                            return true;
                        }
                    }
                }
            }
            kind if kind.is_statement() || kind.is_function_like() => return false,
            _ => {}
        }
    }
    false
}
