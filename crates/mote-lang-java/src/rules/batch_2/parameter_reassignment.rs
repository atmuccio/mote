//! `java/parameter-reassignment` — assignments to method and constructor parameters.

use super::util::{field, FIELDS};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["readability", "confusing"],
    remediation_minutes: 5,
    description: r#"Detects assignments (`=`, compound assignments, `++`, `--`) to a parameter of
the enclosing method or constructor.

**Why it matters.** Readers expect a parameter to hold the value the caller passed for the
whole method. Reassigning it makes the method harder to follow (which value is used where?),
hides the original argument from later code and from debuggers, and is easily mistaken for
changing the caller's variable, which Java never does.

**How to fix.** Introduce a local variable for the computed value. Declaring parameters
`final` prevents the problem.

```java
// Noncompliant
String normalize(String name) {
    name = name.trim();
    return name.toLowerCase();
}

// Compliant
String normalize(String name) {
    String trimmed = name.trim();
    return trimmed.toLowerCase();
}
```

Not reported: lambda parameters, assignments in record compact constructors, where
reassigning the parameters is the intended way to normalize component values, and `p = p`
(reported by `java/self-assignment`)."#,
    // Stylistic and noisy on real code: opt-in (`extends: all` or enable explicitly).
    default_enabled: false,
    ..RuleMeta::new(
        "java/parameter-reassignment",
        "java",
        "Parameter reassigned",
    )
};

#[derive(Default)]
pub struct ParameterReassignment<'t> {
    /// The enclosing method found for the ancestors walked so far, by node id (`None`: a lambda,
    /// initializer or class body comes first): nested assignments share their ancestors, and
    /// walking up from each of them was quadratic in the depth.
    methods: FxHashMap<usize, Option<Node<'t>>>,
}

impl<'t> JavaRule<'t> for ParameterReassignment<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["assignment_expression", "update_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let target = if kind(node) == "assignment_expression" {
            field(node, FIELDS.left)
        } else {
            syntax::first_named_child(node)
        };
        let Some(target) = target.map(syntax::strip_parens) else {
            return;
        };
        if kind(target) != "identifier" {
            return;
        }
        let name = ctx.text(target);
        // Parameters cannot be shadowed by locals or lambda parameters in Java, and the walk
        // stops at a class body, so a same-named parameter of the enclosing method is the
        // variable being assigned. Inside a lambda (whose captured variables are effectively
        // final) the name can only be a lambda parameter or a local: skip.
        let mut method = None;
        let mut passed: Vec<usize> = Vec::new();
        for a in ctx.ancestors(node) {
            if let Some(&known) = self.methods.get(&a.id()) {
                method = known;
                break;
            }
            match kind(a) {
                "method_declaration" | "constructor_declaration" => {
                    method = Some(a);
                    break;
                }
                "lambda_expression"
                | "compact_constructor_declaration"
                | "class_body"
                | "interface_body"
                | "enum_body"
                | "enum_body_declarations"
                | "static_initializer"
                | "program" => break,
                _ => passed.push(a.id()),
            }
        }
        for id in passed {
            self.methods.insert(id, method);
        }
        let Some(method) = method else {
            return;
        };
        let is_param = syntax::parameters(method).any(|p| {
            let n = if kind(p) == "spread_parameter" {
                syntax::declarators(p)
                    .next()
                    .and_then(|d| field(d, FIELDS.name))
            } else {
                field(p, FIELDS.name)
            };
            n.is_some_and(|n| ctx.text(n) == name)
        });
        // `p = p;` does nothing: `java/self-assignment` reports it (as the likely typo it is).
        let self_assignment = kind(node) == "assignment_expression"
            && syntax::operator_str(node) == "="
            && node
                .field(Field::Right)
                .map(syntax::strip_parens)
                .is_some_and(|r| kind(r) == "identifier" && ctx.text(r) == name);
        if is_param && !self_assignment {
            ctx.emit(
                target,
                format!("Introduce a local variable instead of reassigning parameter `{name}`"),
            );
        }
    }
}
