//! `java/equals-asymmetric-subclass` — a subclass whose `equals` requires its own type while the
//! superclass's `equals` accepts any instance of the superclass.

use super::util::any_descendant;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["equality"],
    remediation_minutes: 15,
    description: r#"Detects a class whose `equals(Object)` checks `o instanceof Sub`, when its
superclass (declared in the same file) overrides `equals` with `o instanceof Super`.

**Why it matters.** `equals` must be symmetric. The superclass's method accepts any instance of
the superclass, including a `Sub`, so `superObj.equals(subObj)` can be `true`, while the
subclass's method rejects every object that is not a `Sub`, so `subObj.equals(superObj)` is
`false`. Hash sets, maps and `List.contains` then give answers that depend on which object
happens to be asked, and an object can be "found" in one collection and missing from another.

**How to fix.** Decide whether the two types can ever be equal. If not, compare classes with
`getClass() != o.getClass()` in the superclass. If they can, don't override `equals` in the
subclass (or use a `canEqual` handshake so both sides agree).

```java
// Noncompliant
class Point {
    public boolean equals(Object o) {
        return o instanceof Point p && x == p.x && y == p.y;
    }
}
class ColorPoint extends Point {
    public boolean equals(Object o) {
        return o instanceof ColorPoint c && super.equals(c) && color == c.color;
    }
}

// Compliant
class Point {
    public boolean equals(Object o) {
        if (o == null || getClass() != o.getClass()) return false;
        Point p = (Point) o;
        return x == p.x && y == p.y;
    }
}
```

Only classes declared in the same file are compared. Methods that use `getClass()` or a
`canEqual` handshake are symmetric by design and are not reported."#,
    ..RuleMeta::new(
        "java/equals-asymmetric-subclass",
        "java",
        "Subclass equals is not symmetric with its superclass",
    )
};

/// A class of the file that overrides `equals` with an `instanceof <own name>` test.
struct EqualsClass<'t> {
    name: &'t str,
    superclass: Option<&'t str>,
    /// The `instanceof` type node testing the class's own type.
    check: Node<'t>,
}

#[derive(Default)]
pub struct EqualsAsymmetricSubclass<'t> {
    classes: Vec<EqualsClass<'t>>,
}

impl<'t> JavaRule<'t> for EqualsAsymmetricSubclass<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_declaration"]
    }

    fn method_names(&self) -> &'static [&'static str] {
        &["equals"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) = node.field(Field::Name) else {
            return;
        };
        if ctx.text(name) != "equals" {
            return;
        }
        let mut params = syntax::parameters(node);
        let (Some(param), None) = (params.next(), params.next()) else {
            return;
        };
        if kind(param) != "formal_parameter"
            || !param
                .field(Field::Type)
                .is_some_and(|t| matches!(ctx.text(t), "Object" | "java.lang.Object"))
        {
            return;
        }
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        let text = ctx.text(body);
        if !text.contains("instanceof") || text.contains("getClass") || text.contains("canEqual") {
            return;
        }
        let Some(class) = ctx.parent(node).and_then(|b| ctx.parent(b)) else {
            return;
        };
        if kind(class) != "class_declaration" {
            return;
        }
        let Some(class_name) = class.field(Field::Name).map(|n| ctx.text(n)) else {
            return;
        };
        let Some(pname) = param.field(Field::Name).map(|n| ctx.text(n)) else {
            return;
        };
        let mut check = None;
        any_descendant(body, |n| {
            if kind(n) != "instanceof_expression" {
                return false;
            }
            let left_is_param = n
                .field(Field::Left)
                .is_some_and(|l| ctx.text(syntax::strip_parens(l)) == pname);
            if !left_is_param {
                return false;
            }
            let ty = n.field(Field::Right).or_else(|| {
                n.field(Field::Pattern)
                    .and_then(|p| named_children(p).find(|c| kind(*c) != "identifier"))
            });
            let Some(ty) = ty else { return false };
            let base = if kind(ty) == "generic_type" {
                syntax::first_named_child(ty).unwrap_or(ty)
            } else {
                ty
            };
            if ctx.text(base) == class_name {
                check = Some(base);
                return true;
            }
            false
        });
        let Some(check) = check else { return };
        let superclass = class
            .field(Field::Superclass)
            .and_then(syntax::first_named_child)
            .map(|t| {
                let base = if kind(t) == "generic_type" {
                    syntax::first_named_child(t).unwrap_or(t)
                } else {
                    t
                };
                ctx.text(base)
            });
        self.classes.push(EqualsClass {
            name: class_name,
            superclass,
            check,
        });
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        for sub in &self.classes {
            let Some(sup) = sub.superclass else { continue };
            // Names must be unique among the collected classes to be matched safely.
            let mut supers = self.classes.iter().filter(|c| c.name == sup);
            let (Some(parent), None) = (supers.next(), supers.next()) else {
                continue;
            };
            ctx.emit(
                sub.check,
                format!("Make equals symmetric: `{sup}.equals` accepts any `{sup}`, including a `{}`, but this method rejects every object that is not a `{}`", sub.name, sub.name),
            )
            .secondary(parent.check, "the superclass accepts subclasses here");
        }
    }
}
