//! `js/double-assertion` (off by default; part of the `conventions` set)

use crate::prelude::*;

pub struct DoubleAssertion;

pub static DOUBLE_ASSERTION: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["conventions", "typescript"],
    remediation_minutes: 5,
    params: &[ParamSpec {
        name: "unknown",
        default: ParamDefault::Bool(false),
        description: "Also report double assertions through `unknown` (`value as unknown as T`).",
    }],
    description: r#"Flags a type assertion chained through `any` or `{}` to reach another type:
`value as any as T`, `value as {} as T`, `<T><any>value`.

**Why:** TypeScript refuses an assertion between unrelated types; going through `any` forces it
through and silences every check on the way. When such a conversion is really needed (test
doubles, data from an untyped source), `unknown` is the honest intermediate: it says "no type
information here" without switching off checking around it, and it stands out in review. `any`
in the middle additionally lets the value leak as `any` if the outer assertion is later removed.

**Fix:** fix the types so a single assertion (or none) is enough; otherwise go through
`unknown`, ideally after a runtime check.

```ts
const user = payload as any as User;        // flagged
const user = payload as unknown as User;    // compliant (reported only with unknown=true)
const user = parseUser(payload);            // better: validate at runtime
```

With the `unknown` parameter the rule also reports `as unknown as T`, for code bases that want
every double assertion reviewed. Not reported: `as const` as the outer assertion.

Off by default: part of the opt-in `conventions` set.
"#,
    ..RuleMeta::new(
        "js/double-assertion",
        "js",
        "Type assertions should not be chained through 'any'",
    )
};

/// The type of an assertion node and its asserted expression.
fn assertion<'b, 'a>(expr: &'b Expression<'a>) -> Option<(&'b TSType<'a>, Span)> {
    match expr {
        Expression::TSAsExpression(e) => Some((&e.type_annotation, e.span)),
        Expression::TSTypeAssertion(e) => Some((&e.type_annotation, e.span)),
        _ => None,
    }
}

/// The name of an escape-hatch middle type, if `ty` is one.
fn escape_hatch(ty: &TSType<'_>, with_unknown: bool) -> Option<&'static str> {
    match ty {
        TSType::TSAnyKeyword(_) => Some("any"),
        TSType::TSTypeLiteral(lit) if lit.members.is_empty() => Some("{}"),
        TSType::TSUnknownKeyword(_) if with_unknown => Some("unknown"),
        _ => None,
    }
}

fn is_const(ty: &TSType<'_>) -> bool {
    matches!(ty, TSType::TSTypeReference(r)
        if matches!(&r.type_name, TSTypeName::IdentifierReference(id) if id.name == "const"))
}

impl JsRule for DoubleAssertion {
    fn meta(&self) -> &'static RuleMeta {
        &DOUBLE_ASSERTION
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::TSAsExpression, AstType::TSTypeAssertion]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.is_typescript()
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let (outer_type, inner, span) = match node.kind() {
            AstKind::TSAsExpression(e) => (&e.type_annotation, &e.expression, e.span),
            AstKind::TSTypeAssertion(e) => (&e.type_annotation, &e.expression, e.span),
            _ => return,
        };
        let Some((middle, inner_span)) = assertion(inner) else {
            return;
        };
        if is_const(outer_type)
            || matches!(
                outer_type,
                TSType::TSAnyKeyword(_) | TSType::TSUnknownKeyword(_)
            )
        {
            return;
        }
        let Some(name) = escape_hatch(middle, ctx.param_bool("unknown")) else {
            return;
        };
        // From the middle type to the outer type: `any as T` / `T><any`.
        let (a, b) = (middle.span(), outer_type.span());
        let report = Span::new(a.start.min(b.start), a.end.max(b.end));
        let report = if report.start >= span.start
            && report.end <= span.end
            && inner_span.start >= span.start
        {
            report
        } else {
            span
        };
        ctx.emit(
            report,
            format!(
                "Remove the double assertion through '{name}' to '{}'",
                ctx.text(outer_type.span())
            ),
        );
    }
}
