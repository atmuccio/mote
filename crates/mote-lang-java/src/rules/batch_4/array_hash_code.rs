//! `java/array-hash-code` — `arr.hashCode()` / `Objects.hashCode(arr)` hash the identity.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["arrays", "equality"],
    remediation_minutes: 2,
    description: r#"Detects `hashCode()` called on an array, and `Objects.hashCode(array)`.

**Why it matters.** Arrays don't override `hashCode()`: the value is derived from the
array's identity, not its contents. Two arrays with the same elements hash differently, so a
`hashCode()` implementation built this way breaks the `equals`/`hashCode` contract, and
cache keys or checksums computed from it are meaningless.

**How to fix.** Use `Arrays.hashCode(array)` (or `Arrays.deepHashCode` for nested arrays).
Note that `Objects.hash(a, b)` with several arguments is fine for non-array values but also
uses identity for any array argument.

```java
// Noncompliant
public int hashCode() { return 31 * id + tags.hashCode(); }

// Compliant
public int hashCode() { return 31 * id + Arrays.hashCode(tags); }
```

Only expressions whose type the resolver proves to be an array are reported."#,
    ..RuleMeta::new("java/array-hash-code", "java", "hashCode() of an array")
};

pub struct ArrayHashCode;

impl<'t> JavaRule<'t> for ArrayHashCode {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["hashCode"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        // O(1) text tests before touching the tree.
        let text = ctx.text(node);
        if !text.ends_with("hashCode()")
            && !text.starts_with("Objects.hashCode(")
            && !text.starts_with("java.util.Objects.hashCode(")
        {
            return;
        }
        let Some(name) = util::fld(node, F::Name) else {
            return;
        };
        if ctx.text(name) != "hashCode" {
            return;
        }
        let Some(recv) = util::fld(node, F::Object) else {
            return;
        };
        let array = match util::arg_count(node) {
            0 => recv,
            1 => {
                if !util::names_type(ctx, recv, "java.util.Objects") {
                    return;
                }
                let Some(arg) = util::args(node).next() else {
                    return;
                };
                arg
            }
            _ => return,
        };
        if matches!(kind(array), "this" | "super") {
            return;
        }
        if !ctx.type_of(array).is_some_and(|t| t.is_array()) {
            return;
        }
        ctx.emit(
            name,
            "Use Arrays.hashCode(..) to hash the elements; an array's own hashCode() is based on its identity",
        );
    }
}
