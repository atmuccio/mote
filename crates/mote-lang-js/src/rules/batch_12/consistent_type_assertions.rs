//! `js/consistent-type-assertions` (off by default; part of the `conventions` set)

use crate::prelude::*;

pub struct ConsistentTypeAssertions;

pub static CONSISTENT_TYPE_ASSERTIONS: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["conventions", "typescript"],
    remediation_minutes: 2,
    params: &[
        ParamSpec {
            name: "angle_bracket",
            default: ParamDefault::Bool(true),
            description: "Report the angle-bracket form `<T>value` (use `value as T`).",
        },
        ParamSpec {
            name: "object_literal",
            default: ParamDefault::Bool(true),
            description: "Report type assertions on object literals (`{ … } as T`; annotate instead).",
        },
        ParamSpec {
            name: "object_literal_in_tests",
            default: ParamDefault::Bool(false),
            description: "Also report object-literal assertions in test files, where partial objects asserted to a full type are the usual way to build test doubles.",
        },
        ParamSpec {
            name: "allow_as_argument",
            default: ParamDefault::Bool(false),
            description: "Accept an object-literal assertion passed directly as a call or `new` argument, where no annotation can be written.",
        },
    ],
    description: r#"Flags two forms of TypeScript type assertion: the angle-bracket form `<T>value`, and an
assertion applied to an object literal (`{ … } as T`).

**Why:** `<T>value` and `value as T` mean the same, but the angle-bracket form cannot be used
in `.tsx` files (it reads as JSX) and is easy to mistake for a generic call; one form keeps the
code uniform. An assertion on an object literal is worse than an annotation: `as T` tells the
compiler to trust the author, so a missing property or a misspelled one is not reported, while
`const config: T = { … }` checks the literal against `T` and catches both.

**Fix:** write `value as T`; give object literals a type annotation (or use `satisfies T`).

```ts
const el = <HTMLInputElement>event.target;      // flagged
const el = event.target as HTMLInputElement;    // compliant

const options = { retries: 3 } as Options;      // flagged: a typo in a key would go unnoticed
const options: Options = { retries: 3 };        // compliant
const options = { retries: 3 } satisfies Options; // compliant
```

Not reported: `as const` (and `<const>` is reported only as the angle-bracket form), assertions
to `any`, `unknown` or `never`, object-literal assertions in test files (partial test doubles;
the `object_literal_in_tests` parameter turns them on), and an object literal whose assertion
is itself asserted again (that is a double assertion, see `js/double-assertion`). The
`allow_as_argument` parameter accepts object literals asserted directly as a call argument.

Off by default: part of the opt-in `conventions` set.
"#,
    ..RuleMeta::new(
        "js/consistent-type-assertions",
        "js",
        "Type assertions should use 'as' and should not be applied to object literals",
    )
};

/// `const`, `any` or `unknown`: the target types for which an assertion on an object literal
/// is not a hidden annotation.
fn exempt_target(ty: &TSType<'_>) -> bool {
    match ty {
        TSType::TSAnyKeyword(_) | TSType::TSUnknownKeyword(_) | TSType::TSNeverKeyword(_) => true,
        TSType::TSTypeReference(r) => {
            matches!(&r.type_name, TSTypeName::IdentifierReference(id) if id.name == "const")
        }
        _ => false,
    }
}

impl JsRule for ConsistentTypeAssertions {
    fn meta(&self) -> &'static RuleMeta {
        &CONSISTENT_TYPE_ASSERTIONS
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::TSAsExpression, AstType::TSTypeAssertion]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.is_typescript()
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let (expression, type_annotation, angle) = match node.kind() {
            AstKind::TSAsExpression(e) => (&e.expression, &e.type_annotation, None),
            AstKind::TSTypeAssertion(e) => (&e.expression, &e.type_annotation, Some(e)),
            _ => return,
        };
        if matches!(expression, Expression::ObjectExpression(_))
            && !exempt_target(type_annotation)
            && !matches!(
                ctx.parent_kind(node),
                Some(AstKind::TSAsExpression(_) | AstKind::TSTypeAssertion(_))
            )
            && ctx.param_bool("object_literal")
            && (!ctx.is_test() || ctx.param_bool("object_literal_in_tests"))
            && !(ctx.param_bool("allow_as_argument") && is_argument(ctx, node))
        {
            ctx.emit(
                type_annotation.span(),
                format!(
                    "Annotate the object literal with '{}' instead of asserting its type",
                    ctx.text(type_annotation.span())
                ),
            );
            return;
        }
        if let Some(assertion) = angle {
            if !ctx.param_bool("angle_bracket") {
                return;
            }
            let start = assertion.span.start;
            let end = assertion.expression.span().start.max(start);
            ctx.emit(
                Span::new(start, end),
                "Use the 'as' form instead of the angle-bracket type assertion",
            );
        }
    }
}

fn is_argument(ctx: &Ctx<'_>, node: &AstNode<'_>) -> bool {
    match ctx.parent_kind(node) {
        Some(AstKind::CallExpression(call)) => {
            call.arguments.iter().any(|a| a.span() == node.span())
        }
        Some(AstKind::NewExpression(new)) => new.arguments.iter().any(|a| a.span() == node.span()),
        _ => false,
    }
}
