//! `js/prefer-for-of` (off by default; part of the `conventions` set)

use crate::prelude::*;

pub struct PreferForOf;

pub static PREFER_FOR_OF: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["conventions", "es2015"],
    remediation_minutes: 2,
    description: r#"Flags index loops (`for (let i = 0; i < list.length; i++)`) whose counter is only used to
read the current element (`list[i]`).

**Why:** when the index itself is never needed, the counter, the bound and the subscript are
three places to get wrong (`<=` for `<`, the wrong array in the subscript, a counter changed in
the body) for no benefit. `for…of` says directly that each element is visited once, in order.

**Fix:** loop over the elements.

```js
for (let i = 0; i < items.length; i++) {      // flagged
  total += items[i].price;
}

for (const item of items) {                   // compliant
  total += item.price;
}

for (let i = 0; i < items.length; i++) {      // compliant: the index is used
  console.log(i, items[i]);
}
```

Only loops of exactly this shape are reported: the counter starts at 0, is compared with `<`
to the `length` of a variable or a property path, goes up by one, and every use of it in the body
is a read of that same `list[i]`. Not reported when the element is assigned, incremented or
deleted (`list[i] = …`), called as a method (`list[i]()`, which would lose `list` as `this`),
or when the counter is used anywhere else, including after the loop. Note that for strings,
`for…of` visits code points rather than UTF-16 units.

Off by default: part of the opt-in `conventions` set.
"#,
    ..RuleMeta::new(
        "js/prefer-for-of",
        "js",
        "Index loops that only read the current element should be 'for…of' loops",
    )
};

/// `list` in `list.length` when it is an identifier or a static property path (`this.items`).
fn length_target<'a>(expr: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(m) = expr else {
        return None;
    };
    if m.property.name != "length" || !is_simple_path(&m.object) {
        return None;
    }
    Some(&m.object)
}

fn is_simple_path(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::Identifier(_) | Expression::ThisExpression(_) => true,
        Expression::StaticMemberExpression(m) => !m.optional && is_simple_path(&m.object),
        _ => false,
    }
}

/// The update adds one to `name`: `i++`, `++i`, `i += 1`, `i = i + 1`.
fn increments(update: &Expression<'_>, name: &str) -> bool {
    match update {
        Expression::UpdateExpression(u) => {
            u.operator == UpdateOperator::Increment
                && matches!(&u.argument, SimpleAssignmentTarget::AssignmentTargetIdentifier(id) if id.name == name)
        }
        Expression::AssignmentExpression(a) => {
            let AssignmentTarget::AssignmentTargetIdentifier(target) = &a.left else {
                return false;
            };
            if target.name != name {
                return false;
            }
            match a.operator {
                AssignmentOperator::Addition => helpers::number_value(&a.right) == Some(1.0),
                AssignmentOperator::Assign => match &a.right {
                    Expression::BinaryExpression(b) if b.operator == BinaryOperator::Addition => {
                        (b.left.is_specific_id(name)
                            && helpers::number_value(&b.right) == Some(1.0))
                            || (b.right.is_specific_id(name)
                                && helpers::number_value(&b.left) == Some(1.0))
                    }
                    _ => false,
                },
                _ => false,
            }
        }
        _ => false,
    }
}

impl JsRule for PreferForOf {
    fn meta(&self) -> &'static RuleMeta {
        &PREFER_FOR_OF
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::ForStatement]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::ForStatement(stmt) = node.kind() else {
            return;
        };
        let Some(ForStatementInit::VariableDeclaration(decl)) = &stmt.init else {
            return;
        };
        let [declarator] = decl.declarations.as_slice() else {
            return;
        };
        let BindingPattern::BindingIdentifier(counter) = &declarator.id else {
            return;
        };
        if !declarator
            .init
            .as_ref()
            .is_some_and(|init| helpers::number_value(init) == Some(0.0))
        {
            return;
        }
        let name = counter.name.as_str();
        let Some(Expression::BinaryExpression(test)) = &stmt.test else {
            return;
        };
        if test.operator != BinaryOperator::LessThan || !test.left.is_specific_id(name) {
            return;
        }
        let Some(list) = length_target(&test.right) else {
            return;
        };
        if !stmt.update.as_ref().is_some_and(|u| increments(u, name)) {
            return;
        }
        let list_text = ctx.text(list.span());
        let Some(symbol) = ctx.binding_symbol(counter) else {
            return;
        };
        let (test_span, update_span) = (
            test.span,
            stmt.update.as_ref().map_or(Span::default(), GetSpan::span),
        );
        let body_span = stmt.body.span();
        let mut element_reads = 0;
        for reference in ctx.references_of(symbol) {
            let ref_node = ctx.node(reference.node_id());
            let at = ref_node.span();
            if contains(test_span, at) || contains(update_span, at) {
                continue;
            }
            if !contains(body_span, at) || reference.is_write() {
                return;
            }
            let Some(member_node) = ctx.parent(ref_node) else {
                return;
            };
            let AstKind::ComputedMemberExpression(member) = member_node.kind() else {
                return;
            };
            if member.expression.span() != at || ctx.text(member.object.span()) != list_text {
                return;
            }
            if !read_only_use(ctx, member_node) {
                return;
            }
            element_reads += 1;
        }
        if element_reads == 0 {
            return;
        }
        ctx.emit(
            Span::new(stmt.span.start, stmt.span.start + 3),
            format!("Use a 'for…of' loop over '{list_text}': the index is only used to read its elements"),
        );
    }
}

fn contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end && outer.end > outer.start
}

/// `list[i]` is only read: not assigned, updated, deleted, destructured into or called.
fn read_only_use(ctx: &Ctx<'_>, member: &AstNode<'_>) -> bool {
    let span = member.span();
    match ctx.parent_kind(member) {
        Some(AstKind::AssignmentExpression(a)) => a.left.span() != span,
        Some(AstKind::UpdateExpression(_)) => false,
        Some(AstKind::UnaryExpression(u)) => u.operator != UnaryOperator::Delete,
        Some(AstKind::CallExpression(call)) => call.callee.span() != span,
        Some(AstKind::TaggedTemplateExpression(t)) => t.tag.span() != span,
        Some(
            AstKind::ArrayAssignmentTarget(_)
            | AstKind::ObjectAssignmentTarget(_)
            | AstKind::AssignmentTargetWithDefault(_)
            | AstKind::AssignmentTargetPropertyProperty(_)
            | AstKind::AssignmentTargetRest(_)
            | AstKind::ForInStatement(_)
            | AstKind::ForOfStatement(_),
        ) => false,
        _ => true,
    }
}
