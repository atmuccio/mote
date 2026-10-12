//! `js/no-explicit-any` — a TypeScript-only rule using `applies_to` and a parameter. Off by
//! default: whether `any` is allowed is a team decision.

use crate::prelude::*;

pub struct NoExplicitAny;

pub static NO_EXPLICIT_ANY: RuleMeta = RuleMeta {
    severity: Severity::Low,
    default_enabled: false,
    tags: &["typescript"],
    remediation_minutes: 5,
    params: &[ParamSpec {
        name: "ignore_rest_args",
        default: ParamDefault::Bool(false),
        description: "Allow `any` in the type of a rest parameter (`...args: any[]`).",
    }],
    description: r#"Flags explicit `any` in TypeScript type positions.

Off by default; turn it on if your team forbids `any`. Many TypeScript code bases allow it on
purpose (in tests, at untyped library boundaries, while migrating from JavaScript), and there it
would be most of a first scan's findings. `mote migrate --from eslint` and `mote init
--from eslint` turn it on when the ESLint configuration enables
`@typescript-eslint/no-explicit-any`, directly or through typescript-eslint's `recommended`,
`strict` or `all` configurations.

**Why:** `any` switches the type checker off for everything the value touches: property accesses,
calls, and assignments are all accepted, and the lack of checking spreads to whatever the value flows
into. Bugs the compiler would have caught reach runtime.

**Fix:** use the real type, a generic type parameter, or `unknown` (which forces a check before use).

```ts
function parse(input: any) {}        // flagged
const data = JSON.parse(text) as any; // flagged

function parse(input: string) {}     // compliant
function first<T>(items: T[]): T {}  // compliant
function handle(value: unknown) {}   // compliant
```
"#,
    ..RuleMeta::new("js/no-explicit-any", "js", "Avoid the any type")
};

impl JsRule for NoExplicitAny {
    fn meta(&self) -> &'static RuleMeta {
        &NO_EXPLICIT_ANY
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::TSAnyKeyword]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.is_typescript()
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        if ctx.param_bool("ignore_rest_args") && is_rest_parameter_type(ctx, node) {
            return;
        }
        ctx.emit(
            node.span(),
            "Replace 'any' with a specific type, a type parameter, or 'unknown'",
        );
    }
}

/// `...args: any[]`, `...args: readonly any[]`, `...args: Array<any>`, `...args: ReadonlyArray<any>`.
fn is_rest_parameter_type(ctx: &Ctx<'_>, node: &AstNode<'_>) -> bool {
    let mut ancestors = ctx.ancestors(node);
    for ancestor in ancestors.by_ref() {
        match ancestor.kind() {
            AstKind::TSArrayType(_)
            | AstKind::TSTypeOperator(_)
            | AstKind::TSTypeParameterInstantiation(_) => {}
            AstKind::TSTypeReference(r) => {
                let is_array = matches!(&r.type_name, TSTypeName::IdentifierReference(id)
                    if id.name == "Array" || id.name == "ReadonlyArray");
                if !is_array {
                    return false;
                }
            }
            AstKind::TSTypeAnnotation(_) => break,
            _ => return false,
        }
    }
    matches!(
        ancestors.next().map(AstNode::kind),
        Some(AstKind::FormalParameterRest(_))
    )
}
