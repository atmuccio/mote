//! `js/no-misused-new`

use crate::prelude::*;

pub struct NoMisusedNew;

pub static NO_MISUSED_NEW: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["typescript", "suspicious"],
    remediation_minutes: 5,
    description: r#"Flags constructor declarations in the wrong place: a `constructor` method in an
interface, a construct signature `new (): I` in an interface `I` that returns the interface itself,
and a bodiless class method named `new` that returns the class.

**Why:** these look like constructors but are not. `constructor()` in an interface declares an
ordinary method that happens to be called "constructor". `new (): I` inside `interface I` says that
instances of `I` can themselves be called with `new` to produce another `I` — almost never true. A
class method `new(): C` is a normal method named `new`, not the class constructor. The type checker
accepts all three, so the mistake only shows up when the declared shape does not match reality.

**Fix:** declare constructors in classes with `constructor(...)`. To describe a constructor
function, put the construct signature on a separate type describing the class itself
(`interface WidgetConstructor { new (): Widget }`).

```ts
interface Widget {
  new (): Widget;          // flagged
  constructor(): void;     // flagged
}

interface WidgetConstructor {
  new (): Widget;          // compliant: constructs a different type
}
class Widget {
  constructor() {}         // compliant
}
```
"#,
    ..RuleMeta::new(
        "js/no-misused-new",
        "js",
        "Constructors should be declared correctly",
    )
};

impl JsRule for NoMisusedNew {
    fn meta(&self) -> &'static RuleMeta {
        &NO_MISUSED_NEW
    }

    fn types(&self) -> &'static [AstType] {
        &[
            AstType::TSConstructSignatureDeclaration,
            AstType::TSMethodSignature,
            AstType::MethodDefinition,
        ]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.is_typescript()
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        match node.kind() {
            AstKind::TSConstructSignatureDeclaration(sig) => {
                let Some(interface) = enclosing_interface(ctx, node) else {
                    return;
                };
                let Some(ret) = &sig.return_type else {
                    return;
                };
                if !is_reference_to(&ret.type_annotation, interface.id.name.as_str()) {
                    return;
                }
                let keyword = Span::new(sig.span.start, sig.span.start + 3);
                let span = if ctx.text(keyword) == "new" {
                    keyword
                } else {
                    sig.span
                };
                ctx.emit(
                    span,
                    format!(
                        "Remove this construct signature: instances of the interface '{}' cannot be constructed",
                        interface.id.name
                    ),
                );
            }
            AstKind::TSMethodSignature(sig) => {
                if sig.computed || !sig.key.is_specific_static_name("constructor") {
                    return;
                }
                let Some(interface) = enclosing_interface(ctx, node) else {
                    return;
                };
                ctx.emit(
                    sig.key.span(),
                    format!(
                        "Remove this 'constructor' from the interface '{}': interfaces cannot declare constructors",
                        interface.id.name
                    ),
                );
            }
            AstKind::MethodDefinition(method) => {
                if method.computed
                    || method.kind != MethodDefinitionKind::Method
                    || method.value.body.is_some()
                    || !method.key.is_specific_static_name("new")
                {
                    return;
                }
                let Some(ret) = &method.value.return_type else {
                    return;
                };
                // MethodDefinition → ClassBody → Class
                let Some(AstKind::Class(class)) =
                    ctx.parent(node).and_then(|body| ctx.parent_kind(body))
                else {
                    return;
                };
                let Some(id) = &class.id else {
                    return;
                };
                if is_reference_to(&ret.type_annotation, id.name.as_str()) {
                    ctx.emit(
                        method.key.span(),
                        format!(
                            "Rename this method or declare a real constructor: 'new' is an ordinary method of the class '{}'",
                            id.name
                        ),
                    );
                }
            }
            _ => {}
        }
    }
}

/// The interface whose body directly contains this signature.
fn enclosing_interface<'a>(
    ctx: &Ctx<'a>,
    node: &AstNode<'a>,
) -> Option<&'a TSInterfaceDeclaration<'a>> {
    let body = ctx.parent(node)?;
    if !matches!(body.kind(), AstKind::TSInterfaceBody(_)) {
        return None;
    }
    match ctx.parent_kind(body)? {
        AstKind::TSInterfaceDeclaration(decl) => Some(decl),
        _ => None,
    }
}

/// `Name` or `Name<…>` as a type.
fn is_reference_to(ty: &TSType<'_>, name: &str) -> bool {
    matches!(ty, TSType::TSTypeReference(r)
        if matches!(&r.type_name, TSTypeName::IdentifierReference(id) if id.name == name))
}
