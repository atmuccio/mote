//! `js/no-mixed-enums`

use crate::prelude::*;

pub struct NoMixedEnums;

pub static NO_MIXED_ENUMS: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["typescript", "confusing"],
    remediation_minutes: 10,
    description: r#"Flags enums that mix string members with numeric members.

**Why:** numeric and string enum members behave differently at runtime: numeric members get a
reverse mapping (`Kind[0] === 'A'`) and accept arbitrary numbers where the enum type is expected,
string members do neither. Code iterating over the enum (`Object.values(Kind)`) or validating input
must then deal with both kinds, which is an easy source of bugs. A mixed enum is usually an
accident, such as a member added later with the wrong kind of value.

**Fix:** make all members strings or all members numbers. If both are needed, split the enum.

Members without an initializer count as numbers. When any initializer is not a plain literal (a
reference or a computed expression) the enum is not checked, because its kind cannot be known
without type information. `const enum`s and `declare enum`s are not checked either: they leave no
enum object at runtime, and are often used as a bag of related compile-time constants.

```ts
enum Level {
  Low = 0,
  High = 'high',     // flagged: string member in a numeric enum
}

enum Level {
  Low = 'low',
  High = 'high',     // compliant
}
```
"#,
    ..RuleMeta::new(
        "js/no-mixed-enums",
        "js",
        "Enums should not mix string and number members",
    )
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Number,
    String,
}

impl JsRule for NoMixedEnums {
    fn meta(&self) -> &'static RuleMeta {
        &NO_MIXED_ENUMS
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
        // `const enum`s leave no runtime object behind (no reverse mapping, nothing to iterate):
        // they are commonly used as a bag of mixed compile-time constants.
        if decl.declare || decl.r#const {
            return;
        }
        let members = &decl.body.members;
        let Some(first) = members.first() else {
            return;
        };
        let mut first_kind = None;
        let mut mismatch = None;
        for member in members {
            let Some(kind) = member_kind(member) else {
                return; // unknown kind: stay silent for the whole enum
            };
            match first_kind {
                None => first_kind = Some(kind),
                Some(k) if k != kind && mismatch.is_none() => mismatch = Some((member, kind)),
                _ => {}
            }
        }
        let Some((member, kind)) = mismatch else {
            return;
        };
        let (this, other) = match kind {
            Kind::String => ("string", "numeric"),
            Kind::Number => ("numeric", "string"),
        };
        ctx.emit(
            member.id.span(),
            format!(
                "Do not mix a {this} member into the {other} enum '{}'",
                decl.id.name
            ),
        )
        .secondary(first.id.span(), format!("First member is {other}"));
    }
}

fn member_kind(member: &TSEnumMember<'_>) -> Option<Kind> {
    let Some(init) = &member.initializer else {
        return Some(Kind::Number);
    };
    match init {
        Expression::StringLiteral(_) => Some(Kind::String),
        Expression::TemplateLiteral(t) if t.expressions.is_empty() => Some(Kind::String),
        Expression::NumericLiteral(_) => Some(Kind::Number),
        Expression::UnaryExpression(u)
            if matches!(
                u.operator,
                UnaryOperator::UnaryNegation | UnaryOperator::UnaryPlus | UnaryOperator::BitwiseNot
            ) && matches!(u.argument, Expression::NumericLiteral(_)) =>
        {
            Some(Kind::Number)
        }
        Expression::BinaryExpression(b)
            if matches!(b.left, Expression::NumericLiteral(_))
                && matches!(b.right, Expression::NumericLiteral(_)) =>
        {
            Some(Kind::Number)
        }
        _ => None,
    }
}
