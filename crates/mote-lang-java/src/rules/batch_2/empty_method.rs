//! `java/empty-method` — methods with an empty body and no explanation.

use super::util::{in_junit3_test_class, Mods};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["suspicious", "documentation"],
    remediation_minutes: 5,
    description: r#"Detects methods whose body is empty and contains no comment.

**Why it matters.** An empty method may be intentional (a no-op hook, an interface method
that does not apply) or a forgotten implementation. Readers cannot tell which, and callers may
silently rely on behaviour that never happens.

**How to fix.** Implement the method, throw `UnsupportedOperationException` if the operation
is not supported, or add a comment inside the body explaining why doing nothing is correct.

```java
// Noncompliant
@Override
public void onShutdown() {}

// Compliant
@Override
public void onShutdown() {
    // Nothing to release: this listener holds no resources.
}
```

Not reported: abstract and native methods, `default` interface methods, constructors,
methods with annotations other than `@Override` (for example AspectJ `@Pointcut` or test
lifecycle hooks, which are empty by design), and bodies followed by a comment on the same
line (`void reset() {} // no state`). Test code is not checked: mocks and stubs consist of
empty methods by design. Besides test files, that includes JUnit 3 test classes kept under a
main source root (a class extending `TestCase` or a `…TestCase` base class)."#,
    ..RuleMeta::new(
        "java/empty-method",
        "java",
        "Empty method without explanation",
    )
};

pub struct EmptyMethod;

impl<'t> JavaRule<'t> for EmptyMethod {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if ctx.is_test() {
            return;
        }
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        if !syntax::is_empty_block(body) || syntax::block_has_comment(body) {
            return;
        }
        if Mods::of(node).has("default")
            || Mods::of(node).has("abstract")
            || Mods::of(node).has("native")
        {
            return;
        }
        let src = ctx.source();
        if syntax::annotations(node).any(|a| syntax::annotation_name(a, src) != "Override") {
            return;
        }
        // A trailing comment on the line of the closing brace explains the empty body.
        if let Some(next) = ctx.next_sibling(node) {
            if syntax::is_comment(next) && next.start_position().row == body.end_position().row {
                return;
            }
        }
        if in_junit3_test_class(ctx, node) {
            return;
        }
        let Some(name) = ctx.name_node(node) else {
            return;
        };
        ctx.emit(
            name,
            format!(
                "Implement method `{}` or add a comment explaining why its body is empty",
                ctx.text(name)
            ),
        );
    }
}
