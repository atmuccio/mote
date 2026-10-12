//! `js/no-extraneous-class` (off by default; part of the `conventions` set)

use crate::prelude::*;

pub struct NoExtraneousClass;

pub static NO_EXTRANEOUS_CLASS: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["conventions", "classes"],
    remediation_minutes: 10,
    description: r#"Flags classes that only hold static members and are used as a namespace.

**Why:** a class whose members are all `static` is never meant to be instantiated: it only groups
functions and constants under a name, which is what a module already does. The class adds a
constructor nobody should call, `this` inside its methods refers to the class (and breaks when a
method is passed around as a callback), and bundlers cannot drop the members that are not used.

**Fix:** turn the static members into exported functions and constants of the module; callers
can still group them with `import * as name from '…'`.

```ts
export class StringUtils {                    // flagged
  static capitalize(s: string) { return s[0].toUpperCase() + s.slice(1); }
  static readonly SEPARATOR = '-';
}

export function capitalize(s: string) { return s[0].toUpperCase() + s.slice(1); }
export const SEPARATOR = '-';                 // compliant
```

A constructor that is private, protected or empty does not count as an instance member (it is
the usual way to block instantiation). Not reported: classes that extend or implement another
type, decorated classes and classes with decorated members (frameworks register and
instantiate them), abstract classes (meant to be extended), empty classes (often used as tokens
or markers), classes merged with an interface or namespace of the same name, classes the same
file uses other than by reading a member (passed as a value such as `imports: [LoggerModule]`,
extended, instantiated or used as a type), and `declare` classes.

Off by default: part of the opt-in `conventions` set.
"#,
    ..RuleMeta::new(
        "js/no-extraneous-class",
        "js",
        "Classes should not be used as namespaces for static members",
    )
};

/// A constructor that blocks or does nothing: not an instance member.
fn trivial_constructor(method: &MethodDefinition<'_>) -> bool {
    matches!(
        method.accessibility,
        Some(TSAccessibility::Private | TSAccessibility::Protected)
    ) || (method.value.params.items.is_empty()
        && method.value.params.rest.is_none()
        && method
            .value
            .body
            .as_ref()
            .is_some_and(|b| b.statements.is_empty()))
}

impl JsRule for NoExtraneousClass {
    fn meta(&self) -> &'static RuleMeta {
        &NO_EXTRANEOUS_CLASS
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::Class]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::Class(class) = node.kind() else {
            return;
        };
        if class.declare
            || class.r#abstract
            || class.super_class.is_some()
            || !class.implements.is_empty()
            || !class.decorators.is_empty()
        {
            return;
        }
        let mut statics = 0;
        for element in &class.body.body {
            let is_static = match element {
                ClassElement::StaticBlock(_) => true,
                ClassElement::MethodDefinition(m) => {
                    if !m.decorators.is_empty()
                        || m.value
                            .params
                            .items
                            .iter()
                            .any(|p| !p.decorators.is_empty())
                    {
                        return; // framework metadata on the class
                    }
                    if m.kind == MethodDefinitionKind::Constructor {
                        if trivial_constructor(m) {
                            continue;
                        }
                        return;
                    }
                    m.r#static
                }
                ClassElement::PropertyDefinition(p) => {
                    if !p.decorators.is_empty() {
                        return;
                    }
                    p.r#static
                }
                ClassElement::AccessorProperty(p) => {
                    if !p.decorators.is_empty() {
                        return;
                    }
                    p.r#static
                }
                ClassElement::TSIndexSignature(s) => {
                    if s.r#static {
                        continue; // a type-level declaration only
                    }
                    return;
                }
            };
            if !is_static {
                return;
            }
            statics += 1;
        }
        if statics == 0 || ctx.in_ambient_context(node) {
            return;
        }
        let Some(id) = &class.id else {
            return; // anonymous: usually assigned to a name the rule cannot follow
        };
        if let Some(symbol) = ctx.binding_symbol(id) {
            if !ctx.scoping().symbol_redeclarations(symbol).is_empty() {
                return; // merged with an interface or namespace
            }
            // Used as a namespace: every use reads a member (`Utils.run()`). A class passed
            // around as a value (`imports: [LoggerModule]`), extended, instantiated or used as a
            // type is something else.
            for reference in ctx.references_of(symbol) {
                let ref_node = ctx.node(reference.node_id());
                let as_object = match ctx.parent_kind(ref_node) {
                    Some(AstKind::StaticMemberExpression(m)) => m.object.span() == ref_node.span(),
                    Some(AstKind::ComputedMemberExpression(m)) => {
                        m.object.span() == ref_node.span()
                    }
                    Some(AstKind::ExportSpecifier(_) | AstKind::ExportDefaultDeclaration(_)) => {
                        true
                    }
                    _ => false,
                };
                if !as_object {
                    return;
                }
            }
        }
        ctx.emit(
            id.span,
            format!(
                "Replace the class '{}' of static members with module-level functions and constants",
                id.name
            ),
        );
    }
}
