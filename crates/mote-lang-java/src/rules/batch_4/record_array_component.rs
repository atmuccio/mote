//! `java/record-array-component` — a record with an array component and generated equality.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["equality", "records", "arrays"],
    remediation_minutes: 10,
    description: r#"Detects a `record` with an array component (`int[] values`, `String... parts`)
that does not declare both `equals(Object)` and `hashCode()`.

**Why it matters.** The methods a record generates compare each component with `equals`,
and arrays inherit identity equality from `Object`. Two records holding arrays with the same
contents are therefore *not* equal, hash differently, and can't be found in sets or used as
map keys — the opposite of what a value type promises. `toString()` prints the array's
identity (`[I@1b6d3586`) too.

**How to fix.** Override `equals` and `hashCode` (and usually `toString`) with
`Arrays.equals` / `Arrays.hashCode` / `Arrays.toString`, or use an immutable `List`
component instead of an array.

```java
// Noncompliant
record Samples(String name, double[] values) { }

// Compliant
record Samples(String name, List<Double> values) { }
```

Records that declare both `equals` and `hashCode` are not reported."#,
    ..RuleMeta::new(
        "java/record-array-component",
        "java",
        "Record with an array component uses identity equality",
    )
};

pub struct RecordArrayComponent;

impl<'t> JavaRule<'t> for RecordArrayComponent {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["record_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(params) = util::fld(node, F::Parameters) else {
            return;
        };
        let array_component = named_children(params).find_map(|p| match kind(p) {
            "formal_parameter" => {
                let ty = util::fld(p, F::Type)?;
                let dims = named_children(p).any(|c| kind(c) == "dimensions");
                (kind(ty) == "array_type" || dims).then(|| util::fld(p, F::Name))?
            }
            "spread_parameter" => named_children(p)
                .find(|c| kind(*c) == "variable_declarator")
                .and_then(|d| util::fld(d, F::Name)),
            _ => None,
        });
        let Some(component) = array_component else {
            return;
        };
        let Some(body) = util::fld(node, F::Body) else {
            return;
        };
        let src = ctx.source();
        let (mut has_equals, mut has_hash) = (false, false);
        for m in ctx
            .members(body)
            .filter(|m| kind(*m) == "method_declaration")
        {
            match syntax::name_of(m, src) {
                "equals" if syntax::parameters(m).count() == 1 => has_equals = true,
                "hashCode" if syntax::parameters(m).count() == 0 => has_hash = true,
                _ => {}
            }
        }
        if has_equals && has_hash {
            return;
        }
        let name = ctx.text(component);
        ctx.emit(
            component,
            format!("Override equals() and hashCode() in this record or replace array component `{name}` with a List: arrays are compared by identity"),
        );
    }
}
