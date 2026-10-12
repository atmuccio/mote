//! `js/const-enum` (off by default; part of the `conventions` set)

use crate::prelude::*;

pub struct ConstEnum;

pub static CONST_ENUM: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["conventions", "typescript"],
    remediation_minutes: 2,
    description: r#"Flags `const enum` declarations.

**Why:** a `const enum` has no runtime object: the compiler replaces every use with the literal
value, which needs the declaration's source at compile time. Tools that compile one file at a
time (Babel, esbuild, SWC, `isolatedModules`, type stripping in Node) cannot do that across
files, so exported `const enum`s either fail to build or silently produce different code than
`tsc`. Their values also disappear from the emitted JavaScript, which surprises callers that
look the enum up at run time.

**Fix:** use a regular `enum`, or a `const` object with `as const` and a union type of its
values.

```ts
export const enum Direction { Up, Down }     // flagged
export enum Direction { Up, Down }           // compliant
export const Direction = { Up: 0, Down: 1 } as const;   // compliant
```

Not reported: `declare const enum` and enums inside `declare module` / `declare global` (they
describe code that exists elsewhere).

Off by default: part of the opt-in `conventions` set.
"#,
    ..RuleMeta::new(
        "js/const-enum",
        "js",
        "Enums should not be declared 'const'",
    )
};

impl JsRule for ConstEnum {
    fn meta(&self) -> &'static RuleMeta {
        &CONST_ENUM
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::TSEnumDeclaration]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.is_typescript()
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::TSEnumDeclaration(decl) = node.kind() else {
            return;
        };
        if !decl.r#const || decl.declare || ctx.in_ambient_context(node) {
            return;
        }
        ctx.emit(
            decl.id.span,
            format!(
                "Declare '{}' as a regular 'enum' instead of a 'const enum'",
                decl.id.name
            ),
        );
    }
}
