//! `js/no-const-assign`

use super::util;
use crate::prelude::*;

pub struct NoConstAssign;

pub static NO_CONST_ASSIGN: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["suspicious"],
    remediation_minutes: 2,
    description: r#"Flags assignments, compound assignments (`+=`), and increments of a variable declared with
`const`.

**Why:** a `const` binding cannot be changed. Every such write throws a `TypeError` when it runs
(TypeScript rejects it at compile time, but plain JavaScript only fails at run time — often on a
rarely taken path).

**Fix:** declare the variable with `let` if it really must change, or assign the new value to a
different variable.

```js
const total = 0;
for (const item of items) {
  total += item.price;       // flagged: throws TypeError
}

let total = 0;               // compliant
for (const item of items) {
  total += item.price;
}
```

Changing the *contents* of a constant object or array (`list.push(x)`, `config.debug = true`) is
fine and not reported: only the binding itself is constant.
"#,
    ..RuleMeta::new(
        "js/no-const-assign",
        "js",
        "Constants should not be reassigned",
    )
};

impl JsRule for NoConstAssign {
    fn meta(&self) -> &'static RuleMeta {
        &NO_CONST_ASSIGN
    }

    fn run_once<'a>(&self, ctx: &mut Ctx<'a>) {
        let mut reports = Vec::new();
        for (reference, symbol) in util::written_references(ctx, false) {
            let flags = ctx.symbol_flags(symbol);
            if flags.contains(SymbolFlags::ConstVariable) && !flags.contains(SymbolFlags::Ambient) {
                reports.push((ctx.node(reference.node_id()).span(), symbol));
            }
        }
        for (span, symbol) in reports {
            let name = ctx.symbol_name(symbol);
            ctx.emit(
                span,
                format!("Do not reassign the constant '{name}': this throws a TypeError at run time; declare it with 'let' if it must change"),
            );
        }
    }
}
