//! `js/jsx-no-unstable-key`

use super::util;
use crate::prelude::*;

pub struct JsxNoUnstableKey;

pub static JSX_NO_UNSTABLE_KEY: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["react", "performance"],
    remediation_minutes: 5,
    description: r#"Flags JSX `key` props computed from a fresh random or time-based value on every render:
`Math.random()`, `Date.now()`, `new Date()`, `crypto.randomUUID()`, and the id generators of the
`uuid`, `nanoid`, `cuid`, and lodash `uniqueId` packages.

**Why:** React matches elements between renders by their key. A key that changes on every render
never matches, so React throws the old element away and mounts a new one each time: local state,
focus, input contents, and scroll positions are lost, effects run again, and the whole subtree is
rebuilt — a real performance cost in lists.

**Fix:** use an id that belongs to the data (`key={item.id}`). If the data has none, create the ids
once, when the data is loaded or created, and store them with it.

```jsx
items.map((item) => <Row key={Math.random()} item={item} />); // flagged
items.map((item) => <Row key={uuid()} item={item} />);        // flagged

items.map((item) => <Row key={item.id} item={item} />);       // compliant
```

Only React/Preact files are checked, and only calls whose result is certainly new each time: the
globals above and functions imported from those packages. Forcing a remount with a random key is
reported too: change the key only when a remount is wanted (for example, from a counter in state).
"#,
    ..RuleMeta::new(
        "js/jsx-no-unstable-key",
        "js",
        "JSX keys should not be regenerated on every render",
    )
};

/// Packages whose exported functions return a new id on every call.
const ID_PACKAGES: &[&str] = &[
    "uuid",
    "nanoid",
    "nanoid/non-secure",
    "cuid",
    "@paralleldrive/cuid2",
    "short-uuid",
];

impl JsRule for JsxNoUnstableKey {
    fn meta(&self) -> &'static RuleMeta {
        &JSX_NO_UNSTABLE_KEY
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::JSXAttribute]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        (ctx.file_contains(AstType::JSXElement) || ctx.file_contains(AstType::JSXFragment))
            && util::is_react_file(ctx)
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
        if container.expression.as_expression().is_none() {
            return;
        }
        let container_node = ctx.node(container.node_id.get());
        for inner in util::descendants(ctx, container_node) {
            // Code in a nested function does not run when the key is computed.
            if matches!(
                inner.kind(),
                AstKind::ArrowFunctionExpression(_) | AstKind::Function(_)
            ) {
                return;
            }
            let fresh = match inner.kind() {
                AstKind::CallExpression(call) => fresh_call(ctx, call),
                AstKind::NewExpression(new) => {
                    ctx.is_global(&new.callee, "Date") && new.arguments.is_empty()
                }
                _ => false,
            };
            if fresh {
                let shown = util::first_line(ctx, inner.span());
                ctx.emit(
                    inner.span(),
                    format!(
                        "Use a stable id from the data as the key: '{}' produces a new key on every render, so React remounts the element each time",
                        ctx.text(shown)
                    ),
                );
                return;
            }
        }
    }
}

/// `Math.random()`, `Date.now()`, `performance.now()`, `crypto.randomUUID()`, or a function
/// imported from an id-generating package (`uuid()`, `v4()`, `nanoid()`, `_.uniqueId()`).
fn fresh_call(ctx: &Ctx<'_>, call: &CallExpression<'_>) -> bool {
    match helpers::unwrap_expr(&call.callee) {
        Expression::StaticMemberExpression(m) => {
            let method = m.property.name.as_str();
            match method {
                "random" => ctx.is_global(&m.object, "Math"),
                "now" => {
                    ctx.is_global(&m.object, "Date") || ctx.is_global(&m.object, "performance")
                }
                "randomUUID" => {
                    ctx.is_global(&m.object, "crypto")
                        || matches!(helpers::unwrap_expr(&m.object), Expression::StaticMemberExpression(o)
                            if o.property.name == "crypto")
                }
                "uniqueId" => imported_from(ctx, &m.object, &["lodash", "underscore", "lodash-es"]),
                _ => GENERATORS.contains(&method) && imported_from(ctx, &m.object, ID_PACKAGES),
            }
        }
        callee @ Expression::Identifier(_) => {
            imported_from(ctx, callee, ID_PACKAGES)
                || imported_from(ctx, callee, &["lodash/uniqueId", "lodash.uniqueid"])
                || (imported_name(ctx, callee) == Some("uniqueId")
                    && imported_from(ctx, callee, &["lodash", "lodash-es", "underscore"]))
        }
        _ => false,
    }
}

/// Names of the id generators exported by those packages.
const GENERATORS: &[&str] = &[
    "v1", "v4", "v6", "v7", "nanoid", "createId", "cuid", "generate", "uuid",
];

/// The exported name an identifier was imported as (`None` for default and namespace imports).
fn imported_name<'a>(ctx: &Ctx<'a>, expr: &Expression<'_>) -> Option<&'a str> {
    let Expression::Identifier(id) = helpers::unwrap_expr(expr) else {
        return None;
    };
    let symbol = ctx.reference_symbol(id)?;
    match ctx.symbol_declaration(symbol).kind() {
        AstKind::ImportSpecifier(s) => Some(s.imported.name().as_str()),
        _ => None,
    }
}

/// Is the identifier bound by an import from one of the packages: a generator imported by name
/// (`import { v4 } from 'uuid'`), a default import, or a namespace import?
fn imported_from(ctx: &Ctx<'_>, expr: &Expression<'_>, packages: &[&str]) -> bool {
    let Expression::Identifier(id) = helpers::unwrap_expr(expr) else {
        return false;
    };
    let Some(symbol) = ctx.reference_symbol(id) else {
        return false;
    };
    let decl = ctx.symbol_declaration(symbol);
    match decl.kind() {
        AstKind::ImportSpecifier(s) => {
            let name = s.imported.name();
            if !GENERATORS.contains(&name.as_str()) && name != "uniqueId" {
                return false;
            }
        }
        AstKind::ImportDefaultSpecifier(_) | AstKind::ImportNamespaceSpecifier(_) => {}
        _ => return false,
    }
    ctx.ancestors(decl).any(|n| {
        matches!(n.kind(), AstKind::ImportDeclaration(import)
            if packages.contains(&import.source.value.as_str()))
    })
}
