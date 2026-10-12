//! `js/jsx-no-duplicate-props`

use crate::prelude::*;

pub struct JsxNoDuplicateProps;

pub static JSX_NO_DUPLICATE_PROPS: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["react", "jsx", "suspicious"],
    remediation_minutes: 2,
    description: r#"Flags a JSX element that sets the same prop more than once
(`<input value={a} value={b} />`).

**Why:** only the last value takes effect; the earlier one is silently discarded. A repeated prop
is almost always a merge or copy-paste mistake, and the code does not do what the first value
suggests.

**Fix:** remove one of the props, or rename it to the prop that was meant. Props are compared
exactly (case-sensitive; `xlink:href` style names included). Values coming from a spread
(`{...props}`) are not considered.

```jsx
<Button type="submit" onClick={save} type="button" />  // flagged

<Button type="submit" onClick={save} />                // compliant
<Button {...defaults} type="button" />                 // compliant: overriding a spread is fine
```
"#,
    ..RuleMeta::new(
        "js/jsx-no-duplicate-props",
        "js",
        "JSX elements should not repeat a prop",
    )
};

impl JsRule for JsxNoDuplicateProps {
    fn meta(&self) -> &'static RuleMeta {
        &JSX_NO_DUPLICATE_PROPS
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::JSXOpeningElement]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.file_contains(AstType::JSXOpeningElement)
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::JSXOpeningElement(element) = node.kind() else {
            return;
        };
        if element.attributes.len() < 2 {
            return;
        }
        let source = ctx.source();
        let items = element.attributes.as_slice();
        if items.len() <= 16 {
            // Small elements (the norm): compare with the earlier attributes, no allocation.
            for (i, item) in items.iter().enumerate() {
                let Some((name, span)) = attribute_name(source, item) else {
                    continue;
                };
                let first = items
                    .get(..i)
                    .unwrap_or(&[])
                    .iter()
                    .filter_map(|prev| attribute_name(source, prev))
                    .find(|(n, _)| *n == name);
                if let Some((_, first)) = first {
                    report(ctx, name, span, first);
                }
            }
        } else {
            let mut seen: FxHashMap<&str, Span> = FxHashMap::default();
            for (name, span) in items.iter().filter_map(|a| attribute_name(source, a)) {
                match seen.get(name) {
                    Some(&first) => report(ctx, name, span, first),
                    None => {
                        seen.insert(name, span);
                    }
                }
            }
        }
    }
}

/// Source text of an attribute's name (`value`, `xlink:href`) and its span; `None` for spreads.
fn attribute_name<'a>(source: &'a str, item: &JSXAttributeItem<'_>) -> Option<(&'a str, Span)> {
    match item {
        JSXAttributeItem::Attribute(attr) => {
            let span = attr.name.span();
            let name = source.get(span.start as usize..span.end as usize)?;
            Some((name, span))
        }
        JSXAttributeItem::SpreadAttribute(_) => None,
    }
}

fn report(ctx: &mut Ctx<'_>, name: &str, span: Span, first: Span) {
    ctx.emit(
        span,
        format!("Remove the duplicate prop '{name}': only the last value is used"),
    )
    .secondary(first, "First occurrence");
}
