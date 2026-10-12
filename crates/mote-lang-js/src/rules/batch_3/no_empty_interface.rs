//! `js/no-empty-interface`

use crate::prelude::*;

pub struct NoEmptyInterface;

pub static NO_EMPTY_INTERFACE: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["typescript", "redundant"],
    remediation_minutes: 2,
    description: r#"Flags interfaces that declare no members and extend at most one other type.

**Why:** an empty interface `interface Empty {}` accepts almost any value (it is the same as `{}`),
which is rarely what the author meant. An empty interface that only extends a single type,
`interface Props extends BaseProps {}`, is just another name for that type and adds a declaration
readers must look up.

**Fix:** remove the interface and use the existing type, or write a type alias
(`type Props = BaseProps`). If members are coming later, add them now or leave the interface out
until they exist.

Not flagged: interfaces that extend two or more types (they combine them), interfaces merged with
another declaration of the same name (class/interface merging is a deliberate pattern), interfaces
inside `declare module` / `declare global` blocks (module augmentation), and interfaces whose body
contains a comment explaining why they are empty.

```ts
interface Options {}                         // flagged
interface ButtonProps extends BaseProps {}   // flagged

type ButtonProps = BaseProps;                // compliant
interface Both extends First, Second {}      // compliant: combines two types
```

The same goes for the empty object type `{}` written as the type of a variable, parameter,
property, or return value, or as a type alias: `{}` does not mean "an empty object" but "any value
except `null` and `undefined`" — strings and numbers included. Use `object` for "some object",
`unknown` for "anything", or `Record<string, never>` for "an object with no properties". `{}` is
not reported where it is a deliberate tool: in constraints and intersections (`T extends {}`,
`T & {}`, meaning "not nullish"), directly as a type argument or type-parameter default, and inside
conditional or mapped types. As a member of a union inside a type argument (`Ref<Task | {}>`) it
is reported: the union then accepts any value that is not null or undefined, which defeats the
other members.

```ts
function save(options: {}) {}                // flagged
type Props = {};                             // flagged
function save(options: Record<string, never>) {} // compliant
type NonNullish<T> = T & {};                 // compliant
```
"#,
    ..RuleMeta::new(
        "js/no-empty-interface",
        "js",
        "Interfaces should not be empty",
    )
};

impl JsRule for NoEmptyInterface {
    fn meta(&self) -> &'static RuleMeta {
        &NO_EMPTY_INTERFACE
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::TSInterfaceDeclaration, AstType::TSTypeLiteral]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.is_typescript()
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        if let AstKind::TSTypeLiteral(literal) = node.kind() {
            check_empty_type_literal(ctx, node, literal);
            return;
        }
        let AstKind::TSInterfaceDeclaration(decl) = node.kind() else {
            return;
        };
        if !decl.body.body.is_empty() || decl.extends.len() > 1 {
            return;
        }
        if ctx.has_comments_in(decl.body.span) {
            return;
        }
        // Merged declarations (with a class, another interface, a namespace, …) are deliberate.
        if let Some(symbol) = ctx.binding_symbol(&decl.id) {
            if !ctx.scoping().symbol_redeclarations(symbol).is_empty() {
                return;
            }
        }
        if decl.declare || ctx.in_ambient_context(node) {
            return;
        }
        let name = decl.id.name.as_str();
        let message = match decl.extends.first() {
            None => format!("Remove the empty interface '{name}' or add members to it"),
            Some(parent) => format!(
                "Replace the empty interface '{name}' with a type alias of '{}' or use that type directly",
                heritage_name(ctx, parent)
            ),
        };
        ctx.emit(decl.id.span, message);
    }
}

/// `{}` written as the type of a variable, parameter, property, or return value, or as the body of
/// a type alias (possibly inside a union or an array type). Other positions are left alone
/// because `{}` means something useful there: `T extends {}` and `T & {}` ("not null or
/// undefined"), type arguments and defaults (`Component<{}>`, `<P = {}>`, which fill a required
/// slot), and conditional or mapped types.
fn check_empty_type_literal<'a>(
    ctx: &mut Ctx<'a>,
    node: &AstNode<'a>,
    literal: &TSTypeLiteral<'a>,
) {
    if !literal.members.is_empty() || ctx.has_comments_in(literal.span) {
        return;
    }
    let mut position = None;
    let mut in_union = false;
    for ancestor in ctx.ancestors(node) {
        match ancestor.kind() {
            AstKind::TSUnionType(_) => in_union = true,
            AstKind::TSArrayType(_) => {}
            kind => {
                position = Some(kind);
                break;
            }
        }
    }
    let annotated = match position {
        Some(AstKind::TSTypeAnnotation(_)) => true,
        Some(AstKind::TSTypeAliasDeclaration(alias)) => !alias.declare,
        // `Ref<Task | {}>`: the union swallows the other members; `Array<{}>` alone fills a slot.
        Some(AstKind::TSTypeParameterInstantiation(_)) => in_union,
        _ => false,
    };
    if !annotated || ctx.in_ambient_context(node) {
        return;
    }
    ctx.emit(
        literal.span,
        "Replace the empty object type '{}': it accepts any value except null and undefined; use 'object', 'unknown', or 'Record<string, never>'",
    );
}

/// Source text of an `extends` clause (with its type arguments), on one line and capped in length.
fn heritage_name<'a>(ctx: &Ctx<'a>, heritage: &TSInterfaceHeritage<'a>) -> String {
    let text = ctx.text(heritage.span);
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    one_line.chars().take(60).collect()
}
