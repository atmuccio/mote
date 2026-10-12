//! `java/class-literal-is-instance` — `Foo.class.isInstance(x)` where `x instanceof Foo` says
//! the same.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["idiom", "clarity"],
    remediation_minutes: 1,
    description: r#"Detects `isInstance` called on a class literal: `Foo.class.isInstance(x)`.

**Why it matters.** With a class literal the type is known when the code is written, and
`x instanceof Foo` is the language's own test for it: shorter, checked by the compiler (it
rejects a test that can never succeed, such as a `String` against `Integer`), and able to bind
a pattern variable (`x instanceof Foo foo`) that removes the cast which usually follows.
`Class.isInstance` exists for types only known at run time, held in a `Class` variable.

**How to fix.** Write `x instanceof Foo`.

```java
// Noncompliant
if (Number.class.isInstance(value)) {
    total += ((Number) value).doubleValue();
}

// Compliant
if (value instanceof Number number) {
    total += number.doubleValue();
}
```

Not reported: `isInstance` on a `Class` held in a variable, field or parameter, the method
reference `Foo.class::isInstance` (a ready-made predicate for streams), and primitive class
literals (`int.class.isInstance(x)` is always false: another problem)."#,
    ..RuleMeta::new(
        "java/class-literal-is-instance",
        "java",
        "Class literal isInstance instead of instanceof",
    )
};

pub struct ClassLiteralIsInstance;

impl<'t> JavaRule<'t> for ClassLiteralIsInstance {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["isInstance"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if ctx.call_name(node) != Some("isInstance") || syntax::argument_count(node) != 1 {
            return;
        }
        let Some(recv) = node.field(Field::Object).map(syntax::strip_parens) else {
            return;
        };
        if kind(recv) != "class_literal" {
            return;
        }
        let Some(t) = syntax::first_named_child(recv) else {
            return;
        };
        if matches!(
            kind(t),
            "integral_type" | "floating_point_type" | "boolean_type" | "void_type"
        ) {
            return;
        }
        let Some(arg) = syntax::arguments(node).next() else {
            return;
        };
        let ty = ctx.text(t);
        let arg_text = ctx.text(arg);
        let shown = if arg_text.len() <= 30 && !arg_text.contains('\n') {
            arg_text
        } else {
            "x"
        };
        ctx.emit(
            node,
            format!("Use `{shown} instanceof {ty}` instead of `{ty}.class.isInstance(..)`"),
        );
    }
}
