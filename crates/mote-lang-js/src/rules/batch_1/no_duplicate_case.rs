//! `js/no-duplicate-case`

use super::util;
use crate::prelude::*;

pub struct NoDuplicateCase;

pub static NO_DUPLICATE_CASE: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["suspicious"],
    remediation_minutes: 5,
    description: r#"Flags `case` labels of a `switch` that repeat an earlier label of the same `switch`.

**Why:** `switch` runs the first matching `case`, so a repeated label can never be reached. The
duplicate is usually a copy-paste slip where the new value was not filled in, which means the
value that was meant is not handled at all.

**Fix:** change the duplicated label to the intended value, or merge the two cases.

```js
switch (action.type) {
  case 'add':
    return add(state, action);
  case 'remove':
    return remove(state, action);
  case 'add':                 // flagged: unreachable
    return reset(state);
}
```

Labels are compared by value for literals (`'a'` and `` `a` `` are the same) and by structure for
other side-effect-free expressions; labels that call functions are not compared.
"#,
    ..RuleMeta::new(
        "js/no-duplicate-case",
        "js",
        "Switch case labels should be unique",
    )
};

#[derive(PartialEq, Eq, Hash)]
enum LiteralKey<'a> {
    Str(&'a str),
    Num(u64),
    /// Source text of a plain name or `a.b.c` member chain.
    Name(&'a str),
}

impl JsRule for NoDuplicateCase {
    fn meta(&self) -> &'static RuleMeta {
        &NO_DUPLICATE_CASE
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::SwitchStatement]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::SwitchStatement(switch) = node.kind() else {
            return;
        };
        if switch.cases.len() < 2 {
            return;
        }
        let mut literals: FxHashMap<LiteralKey<'a>, Span> = FxHashMap::default();
        let mut others: Vec<&Expression<'a>> = Vec::new();
        let mut duplicates: Vec<(Span, Span)> = Vec::new();
        for case in &switch.cases {
            let Some(test) = &case.test else {
                continue;
            };
            // `case NaN:` never matches at all (see `js/use-isnan`).
            if matches!(helpers::unwrap_expr(test), Expression::Identifier(id) if id.name == "NaN")
            {
                continue;
            }
            if let Some(key) = literal_key(ctx, test) {
                match literals.get(&key) {
                    Some(first) => duplicates.push((test.span(), *first)),
                    None => {
                        literals.insert(key, test.span());
                    }
                }
                continue;
            }
            if !util::is_pure(test) {
                continue;
            }
            match others.iter().find(|e| util::same_pure_expr(e, test)) {
                Some(first) => duplicates.push((test.span(), first.span())),
                None => others.push(test),
            }
        }
        for (span, first) in duplicates {
            let shown = util::excerpt(ctx.text(span));
            ctx.emit(
                span,
                format!("Remove or change this duplicate case '{shown}': it can never be reached"),
            )
            .secondary(first, "First case with this value");
        }
    }
}

fn literal_key<'a>(ctx: &Ctx<'a>, expr: &Expression<'a>) -> Option<LiteralKey<'a>> {
    if let Some(s) = helpers::string_value(expr) {
        return Some(LiteralKey::Str(s));
    }
    match helpers::unwrap_expr(expr) {
        // `-0` and `0` are the same case (`===`); NaN never matches anything.
        Expression::NumericLiteral(_) | Expression::UnaryExpression(_) => {
            let v = helpers::number_value(expr)?;
            if v.is_nan() {
                None
            } else if v == 0.0 {
                Some(LiteralKey::Num(0))
            } else {
                Some(LiteralKey::Num(v.to_bits()))
            }
        }
        Expression::Identifier(_) | Expression::StaticMemberExpression(_)
            if is_name_chain(expr) =>
        {
            let text = ctx.text(expr.span());
            (!text.is_empty() && !text.contains(|c: char| c.is_whitespace() || c == '/'))
                .then_some(LiteralKey::Name(text))
        }
        _ => None,
    }
}

fn is_name_chain(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::Identifier(_) | Expression::ThisExpression(_) => true,
        Expression::StaticMemberExpression(m) => is_name_chain(&m.object),
        _ => false,
    }
}
