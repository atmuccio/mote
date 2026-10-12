//! `js/no-dupe-keys` — a node-local check that reports a secondary location.

use crate::prelude::*;

pub struct NoDupeKeys;

pub static NO_DUPE_KEYS: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["suspicious"],
    remediation_minutes: 5,
    description: r#"Flags object literals that define the same key more than once.

**Why:** the last definition silently wins, so an earlier value is lost. This is almost always a
copy-paste mistake or a merge error. Keys are compared by their runtime name: `a`, `'a'`, and
`['a']` are the same key, and so are `1`, `1.0`, `0x1`, and `'1'`.

**Fix:** remove one of the definitions or rename the key.

```js
const config = {
  timeout: 1000,
  retries: 3,
  timeout: 5000,          // flagged: overrides the first `timeout`
};

const point = {
  get x() { return this._x; },
  set x(v) { this._x = v; }, // compliant: a getter/setter pair
};
```

Spread elements and computed keys that are not literals are ignored.
"#,
    ..RuleMeta::new(
        "js/no-dupe-keys",
        "js",
        "Object literal keys should be unique",
    )
};

#[derive(Default, Clone, Copy)]
struct Seen {
    init: Option<Span>,
    get: Option<Span>,
    set: Option<Span>,
}

impl JsRule for NoDupeKeys {
    fn meta(&self) -> &'static RuleMeta {
        &NO_DUPE_KEYS
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::ObjectExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::ObjectExpression(object) = node.kind() else {
            return;
        };
        if object.properties.len() < 2 {
            return;
        }
        // Most objects have a few distinct keys: compare them pairwise before sorting a list.
        if object.properties.len() <= SMALL && !has_repeated_key(object) {
            return;
        }
        // (name, source order, kind, key span), sorted so equal names are adjacent.
        let mut keys: Vec<(Cow<'a, str>, usize, PropertyKind, Span)> = object
            .properties
            .iter()
            .filter_map(ObjectPropertyKind::as_property)
            .enumerate()
            .filter_map(|(order, prop)| {
                let name = helpers::property_key_name(&prop.key)?;
                // `__proto__: x` sets the prototype rather than defining a property.
                (name != "__proto__").then_some((name, order, prop.kind, prop.key.span()))
            })
            .collect();
        if keys.len() < 2 {
            return;
        }
        keys.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

        let mut seen = Seen::default();
        for i in 0..keys.len() {
            if i > 0 && keys[i].0 != keys[i - 1].0 {
                seen = Seen::default();
            }
            let (ref name, _, kind, span) = keys[i];
            let first = match kind {
                PropertyKind::Init => seen.init.or(seen.get).or(seen.set),
                PropertyKind::Get => seen.init.or(seen.get),
                PropertyKind::Set => seen.init.or(seen.set),
            };
            let slot = match kind {
                PropertyKind::Init => &mut seen.init,
                PropertyKind::Get => &mut seen.get,
                PropertyKind::Set => &mut seen.set,
            };
            slot.get_or_insert(span);
            if let Some(first) = first {
                let shown: String = name.chars().take(40).collect();
                ctx.emit(
                    span,
                    format!("Remove or rename the duplicate key '{shown}'"),
                )
                .secondary(first, "First definition");
            }
        }
    }
}

/// Objects with at most this many properties are checked pairwise first.
const SMALL: usize = 12;

/// Do two properties (other than `__proto__`) have the same key name?
fn has_repeated_key(object: &ObjectExpression<'_>) -> bool {
    let props = || {
        object
            .properties
            .iter()
            .filter_map(ObjectPropertyKind::as_property)
    };
    // Each key's name once (a numeric key's is formatted), then the pairs.
    let mut names: [Option<Cow<'_, str>>; SMALL] = Default::default();
    let mut count = 0;
    for (slot, prop) in names.iter_mut().zip(props()) {
        *slot = helpers::property_key_name(&prop.key);
        count += 1;
    }
    if props().nth(SMALL).is_none() {
        let names = names.get(..count).unwrap_or(&[]);
        return names.iter().enumerate().any(|(i, a)| {
            let Some(a) = a else {
                return false;
            };
            a != "__proto__"
                && names
                    .get(i + 1..)
                    .unwrap_or(&[])
                    .iter()
                    .any(|b| b.as_deref() == Some(a.as_ref()))
        });
    }
    props().enumerate().any(|(i, a)| {
        let Some(name) = helpers::property_key_name(&a.key) else {
            return false;
        };
        name != "__proto__"
            && props()
                .skip(i + 1)
                .any(|b| helpers::property_key_name(&b.key).is_some_and(|n| n == name))
    })
}
