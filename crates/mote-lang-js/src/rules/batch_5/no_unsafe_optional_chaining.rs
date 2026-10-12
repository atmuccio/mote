//! `js/no-unsafe-optional-chaining`

use crate::prelude::*;
use oxc_span::ContentEq;

pub struct NoUnsafeOptionalChaining;

pub static NO_UNSAFE_OPTIONAL_CHAINING: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Stable,
    tags: &["null-handling", "suspicious"],
    remediation_minutes: 5,
    description: r#"Flags an optional chain (`a?.b`) whose result is used where `undefined` throws: spread into an
array or argument list, destructured, iterated with `for…of`, called or constructed after
parentheses, used as the right side of `in` / `instanceof`, or extended by a class.

**Why:** `?.` exists because the value may be missing; when it is, the whole chain evaluates to
`undefined`. Using that result where a value is mandatory throws a `TypeError` —
`[...list?.items]` or `const { id } = user?.profile` fail exactly in the case the `?.` was written
for. Parentheses end the short-circuit, so `(obj?.method)()` calls `undefined` instead of skipping
the call.

**Fix:** provide a fallback (`[...(list?.items ?? [])]`, `const { id } = user?.profile ?? {}`), keep
the call inside the chain (`obj?.method()`), or drop the `?.` if the value can never be missing.

```js
const all = [...group?.members];          // flagged: TypeError when group is missing
const { name } = config?.user;            // flagged
for (const item of order?.items) {}       // flagged
(api?.refresh)();                          // flagged: calls undefined

const all = [...(group?.members ?? [])];  // compliant
const { name } = config?.user ?? {};      // compliant
api?.refresh();                           // compliant
const copy = { ...config?.user };         // compliant: spreading undefined into an object is fine
```

Not reported: chains whose optional part can never be nullish (`(list || [])?.map(…)`), results
passed through `??` or `||` to a fallback, results asserted with a TypeScript `!` or a type
assertion (`(a?.b as Item).name`), chains in the arguments of an optional call on the same object
(`a?.f([...a?.b])` only runs when `a` exists), chains already tested by an enclosing condition
(`if (a?.b) { for (const x of a?.b) … }`, `a?.b && [...a?.b]`) or whose object that condition
already reads without `?.` (`if (a.kind === 1) { … a?.b … }`), and elements that passed a
`.filter()` testing the same chain (`list.filter((e) => e?.b).map((e) => [...e?.b])`).
"#,
    ..RuleMeta::new(
        "js/no-unsafe-optional-chaining",
        "js",
        "Optional chains should not be used where undefined throws",
    )
};

impl JsRule for NoUnsafeOptionalChaining {
    fn meta(&self) -> &'static RuleMeta {
        &NO_UNSAFE_OPTIONAL_CHAINING
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::ChainExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::ChainExpression(chain) = node.kind() else {
            return;
        };
        let Some(what) = unsafe_use(ctx, node) else {
            return;
        };
        if nullable_bases(&chain.expression).is_empty() || is_guarded(ctx, node, chain) {
            return;
        }
        ctx.emit(
            chain.span,
            format!("This optional chain can be undefined, and {what} throws a TypeError then; add a fallback such as '?? []' or remove the '?.'"),
        );
    }
}

/// How the value of the chain `node` is used, when that use throws on `undefined`.
fn unsafe_use<'a>(ctx: &Ctx<'a>, node: &AstNode<'a>) -> Option<&'static str> {
    let mut current = node;
    loop {
        let parent = ctx.parent(current)?;
        let span = current.span();
        match parent.kind() {
            // `(a?.b as T).c`: the author asserts the type, as with `!`.
            AstKind::TSAsExpression(_) | AstKind::TSTypeAssertion(_) => return None,
            AstKind::TSSatisfiesExpression(_) | AstKind::AwaitExpression(_) => {}
            AstKind::ConditionalExpression(c) => {
                if c.test.span() == span {
                    return None;
                }
            }
            AstKind::LogicalExpression(l) => {
                // `x?.y || fallback` recovers; `a && x?.y` / `fallback ?? x?.y` pass it on.
                if l.left.span() == span && l.operator != LogicalOperator::And {
                    return None;
                }
            }
            AstKind::SequenceExpression(s) => {
                if s.expressions.last().map(GetSpan::span) != Some(span) {
                    return None;
                }
            }
            AstKind::CallExpression(call) => {
                return (call.callee.span() == span && !call.optional).then_some("calling it");
            }
            AstKind::NewExpression(new) => {
                return (new.callee.span() == span).then_some("constructing it");
            }
            AstKind::StaticMemberExpression(m) => {
                return (m.object.span() == span && !m.optional)
                    .then_some("reading a property of it");
            }
            AstKind::ComputedMemberExpression(m) => {
                return (m.object.span() == span && !m.optional)
                    .then_some("reading a property of it");
            }
            AstKind::PrivateFieldExpression(m) => {
                return (m.object.span() == span && !m.optional)
                    .then_some("reading a property of it");
            }
            AstKind::TaggedTemplateExpression(t) => {
                return (t.tag.span() == span).then_some("using it as a template tag");
            }
            AstKind::SpreadElement(_) => {
                return match ctx.parent_kind(parent)? {
                    AstKind::ArrayExpression(_) => Some("spreading it into an array"),
                    AstKind::CallExpression(_) | AstKind::NewExpression(_) => {
                        Some("spreading it into the arguments")
                    }
                    _ => None,
                };
            }
            AstKind::VariableDeclarator(d) => {
                let destructures = d.init.as_ref().is_some_and(|i| i.span() == span)
                    && !matches!(d.id, BindingPattern::BindingIdentifier(_));
                return destructures.then_some("destructuring it");
            }
            AstKind::AssignmentExpression(a) => {
                let destructures = a.right.span() == span
                    && matches!(
                        a.left,
                        AssignmentTarget::ArrayAssignmentTarget(_)
                            | AssignmentTarget::ObjectAssignmentTarget(_)
                    );
                return destructures.then_some("destructuring it");
            }
            AstKind::AssignmentPattern(p) => {
                let destructures = p.right.span() == span
                    && !matches!(p.left, BindingPattern::BindingIdentifier(_));
                return destructures.then_some("destructuring it");
            }
            AstKind::ForOfStatement(f) => {
                return (f.right.span() == span).then_some("iterating over it");
            }
            AstKind::BinaryExpression(b) => {
                let rhs = b.right.span() == span
                    && matches!(b.operator, BinaryOperator::In | BinaryOperator::Instanceof);
                return rhs.then_some(if b.operator == BinaryOperator::In {
                    "using it as the right side of 'in'"
                } else {
                    "using it as the right side of 'instanceof'"
                });
            }
            AstKind::Class(c) => {
                return c
                    .super_class
                    .as_ref()
                    .is_some_and(|s| s.span() == span)
                    .then_some("extending it");
            }
            _ => return None,
        }
        current = parent;
    }
}

/// The expressions in front of each `?.` of the chain that may be nullish: the chain evaluates to
/// `undefined` exactly when one of them is. Empty when every `?.` follows a value that is never
/// nullish (`(list || [])?.map(…)`) or when the chain ends in a non-null assertion (`a?.b!`).
fn nullable_bases<'b, 'a>(element: &'b ChainElement<'a>) -> Vec<&'b Expression<'a>> {
    let mut bases = Vec::new();
    let mut next: Option<&'b Expression<'a>> = match element {
        ChainElement::CallExpression(c) => {
            if c.optional {
                bases.push(&c.callee);
            }
            Some(&c.callee)
        }
        // `a?.b!`: the author asserts the result is present.
        ChainElement::TSNonNullExpression(_) => return bases,
        ChainElement::StaticMemberExpression(m) => {
            if m.optional {
                bases.push(&m.object);
            }
            Some(&m.object)
        }
        ChainElement::ComputedMemberExpression(m) => {
            if m.optional {
                bases.push(&m.object);
            }
            Some(&m.object)
        }
        ChainElement::PrivateFieldExpression(m) => {
            if m.optional {
                bases.push(&m.object);
            }
            Some(&m.object)
        }
    };
    while let Some(expr) = next {
        next = match expr {
            Expression::CallExpression(c) => {
                if c.optional {
                    bases.push(&c.callee);
                }
                Some(&c.callee)
            }
            Expression::TSNonNullExpression(e) => Some(&e.expression),
            Expression::StaticMemberExpression(m) => {
                if m.optional {
                    bases.push(&m.object);
                }
                Some(&m.object)
            }
            Expression::ComputedMemberExpression(m) => {
                if m.optional {
                    bases.push(&m.object);
                }
                Some(&m.object)
            }
            Expression::PrivateFieldExpression(m) => {
                if m.optional {
                    bases.push(&m.object);
                }
                Some(&m.object)
            }
            _ => None,
        };
    }
    bases.retain(|b| !never_nullish(b));
    bases
}

/// Is the chain only evaluated when it cannot short-circuit? Inside the arguments of an optional
/// call on the same object (`a?.f([...a?.b])`), or where a condition has already tested the same
/// chain (`if (a?.b) { for (const x of a?.b) … }`, `a?.b && [...a?.b]`).
fn is_guarded<'a>(ctx: &Ctx<'a>, node: &AstNode<'a>, chain: &ChainExpression<'a>) -> bool {
    let bases = nullable_bases(&chain.expression);
    let text = ctx.text(chain.span);
    let within = |outer: Span, inner: Span| outer.start <= inner.start && inner.end <= outer.end;
    // A condition guards the chain when it tests the same chain, or reads a property of one of
    // its bases without `?.` (`if (cell.kind === 1) { … cell?.outputs … }`).
    let guards = |test: Span| {
        let test = ctx.text(test);
        test.contains(text)
            || bases
                .iter()
                .any(|b| reads_property_of(test, ctx.text(b.span())))
    };
    let mut current = node.span();
    for ancestor in ctx.ancestors(node) {
        match ancestor.kind() {
            AstKind::CallExpression(call) if !within(call.callee.span(), current) => {
                let optional_object = match &call.callee {
                    Expression::StaticMemberExpression(m) if m.optional => Some(&m.object),
                    Expression::ComputedMemberExpression(m) if m.optional => Some(&m.object),
                    _ => None,
                };
                if optional_object.is_some_and(|o| bases.iter().any(|b| b.content_eq(o))) {
                    return true;
                }
                // `list.filter((e) => e?.items).map((e) => [...e?.items])`: the filter only lets
                // elements through whose chain is present.
                if let Expression::StaticMemberExpression(m) = &call.callee {
                    if let Expression::CallExpression(filter) = &m.object {
                        let filtered = matches!(&filter.callee, Expression::StaticMemberExpression(f) if f.property.name == "filter");
                        if filtered
                            && filter
                                .arguments
                                .first()
                                .is_some_and(|cb| ctx.text(cb.span()).contains(text))
                        {
                            return true;
                        }
                    }
                }
            }
            AstKind::IfStatement(s) => {
                if within(s.consequent.span(), current) && guards(s.test.span()) {
                    return true;
                }
            }
            AstKind::ConditionalExpression(c) => {
                if within(c.consequent.span(), current) && guards(c.test.span()) {
                    return true;
                }
            }
            AstKind::LogicalExpression(l) => {
                if l.operator == LogicalOperator::And
                    && within(l.right.span(), current)
                    && guards(l.left.span())
                {
                    return true;
                }
            }
            AstKind::Function(_) | AstKind::Class(_) => return false,
            _ => {}
        }
        current = ancestor.span();
    }
    false
}

/// Values that are never `null` or `undefined`.
fn never_nullish(expr: &Expression<'_>) -> bool {
    match helpers::unwrap_expr(expr) {
        Expression::ArrayExpression(_)
        | Expression::ObjectExpression(_)
        | Expression::StringLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BigIntLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::RegExpLiteral(_)
        | Expression::TemplateLiteral(_)
        | Expression::NewExpression(_)
        | Expression::ClassExpression(_)
        | Expression::FunctionExpression(_)
        | Expression::ArrowFunctionExpression(_) => true,
        Expression::LogicalExpression(l) => match l.operator {
            LogicalOperator::Or | LogicalOperator::Coalesce => never_nullish(&l.right),
            LogicalOperator::And => false,
        },
        Expression::ConditionalExpression(c) => {
            never_nullish(&c.consequent) && never_nullish(&c.alternate)
        }
        _ => false,
    }
}

/// Does `text` read a property of `base` without optional chaining (`base.x`, `base[0]`)? The
/// occurrence must not continue a longer name (`database.x` does not read `base`).
fn reads_property_of(text: &str, base: &str) -> bool {
    if base.is_empty() {
        return false;
    }
    let mut from = 0;
    while let Some(pos) = text.get(from..).and_then(|rest| rest.find(base)) {
        let start = from + pos;
        let end = start + base.len();
        let before = text.get(..start).and_then(|s| s.chars().next_back());
        let boundary =
            !before.is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | '$' | '.' | '?'));
        let after = text.get(end..).and_then(|s| s.chars().next());
        if boundary && matches!(after, Some('.' | '[')) {
            return true;
        }
        from = end;
    }
    false
}
