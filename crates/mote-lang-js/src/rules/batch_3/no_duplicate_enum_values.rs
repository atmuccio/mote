//! `js/no-duplicate-enum-values`

use crate::prelude::*;

pub struct NoDuplicateEnumValues;

pub static NO_DUPLICATE_ENUM_VALUES: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["typescript", "suspicious"],
    remediation_minutes: 5,
    description: r#"Flags enum members initialized with the same literal value as an earlier member
of the same enum.

**Why:** enum members are meant to be distinguishable. When two members share a value, comparisons
and `switch` statements cannot tell them apart, and a reverse lookup (`Color[1]`) returns only the
last name. This is nearly always a copy-paste mistake.

**Fix:** give the member its own value. If a second name for the same value is really wanted, make
the intent explicit by referring to the first member (`Gray = Color.Grey`).

Only explicit string and number literal initializers are compared (`'a'`, `1`, `-1`, `0x1`); members
without an initializer or with computed initializers are ignored. Numeric values are not compared
in `const enum`s or in enums whose name contains `Const` (`Constants`, `LayoutConsts`): those are
typically bags of named numbers — sizes, bit masks, character codes — where equal values are
intended.

```ts
enum Status {
  Active = 1,
  Disabled = 2,
  Deleted = 2,        // flagged: same value as Disabled
}

enum Color {
  Grey = 'grey',
  Gray = Grey,        // compliant: an explicit alias
}
```
"#,
    ..RuleMeta::new(
        "js/no-duplicate-enum-values",
        "js",
        "Enum members should have distinct values",
    )
};

/// A literal enum value: strings by content, numbers by their bit pattern (normalised `-0`).
#[derive(PartialEq, Eq, Hash)]
enum Value<'a> {
    Str(&'a str),
    Num(u64),
}

impl JsRule for NoDuplicateEnumValues {
    fn meta(&self) -> &'static RuleMeta {
        &NO_DUPLICATE_ENUM_VALUES
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
        // Numeric `const enum`s and enums named like `Constants` are usually bags of named
        // numbers (sizes, masks, char codes) in which equal values are deliberate.
        let check_numbers = !decl.r#const && !decl.id.name.contains("Const");
        let value_of = |m: &'a TSEnumMember<'a>| {
            m.initializer
                .as_ref()
                .and_then(literal_value)
                .filter(|v| check_numbers || matches!(v, Value::Str(_)))
        };
        let members = &decl.body.members;
        if members.iter().filter_map(value_of).nth(1).is_none() {
            return;
        }
        let mut seen: FxHashMap<Value<'a>, &'a TSEnumMember<'a>> = FxHashMap::default();
        for member in members {
            let Some(value) = value_of(member) else {
                continue;
            };
            match seen.get(&value) {
                Some(first) => {
                    let name = member_name(ctx, member);
                    let first_name = member_name(ctx, first);
                    ctx.emit(
                        member.id.span(),
                        format!(
                            "Give the enum member '{name}' its own value: '{first_name}' already has it"
                        ),
                    )
                    .secondary(first.id.span(), "First member with this value");
                }
                None => {
                    seen.insert(value, member);
                }
            }
        }
    }
}

fn literal_value<'a>(init: &Expression<'a>) -> Option<Value<'a>> {
    if let Some(s) = helpers::string_value(init) {
        return Some(Value::Str(s));
    }
    let n = match init {
        Expression::NumericLiteral(n) => n.value,
        Expression::UnaryExpression(u)
            if u.operator == UnaryOperator::UnaryNegation
                && matches!(u.argument, Expression::NumericLiteral(_)) =>
        {
            helpers::number_value(init)?
        }
        _ => return None,
    };
    if n.is_nan() {
        return None;
    }
    // `-0` and `0` are the same enum value.
    let n = if n == 0.0 { 0.0 } else { n };
    Some(Value::Num(n.to_bits()))
}

fn member_name<'a>(ctx: &Ctx<'a>, member: &TSEnumMember<'a>) -> &'a str {
    match &member.id {
        TSEnumMemberName::Identifier(id) => id.name.as_str(),
        TSEnumMemberName::String(s) | TSEnumMemberName::ComputedString(s) => s.value.as_str(),
        TSEnumMemberName::ComputedTemplateString(t) => ctx.text(t.span),
    }
}
