//! `js/swapped-arguments`

use crate::prelude::*;

pub struct SwappedArguments;

pub static SWAPPED_ARGUMENTS: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["suspicious"],
    remediation_minutes: 5,
    description: r#"Flags a call that passes two arguments named like the function's parameters, but in each other's
positions: `resize(height, width)` for `function resize(width, height)`.

**Why:** when the names of the arguments match the names of the parameters, the author clearly
meant them to correspond. Passing them crosswise is a classic slip that nothing catches when both
have the same type — the call runs and silently computes the wrong thing.

**Fix:** pass the arguments in the order the function declares, or rename them if the crossing is
intended.

```js
function resize(width, height) { /* … */ }

resize(height, width);          // flagged
resize(size.height, size.width); // flagged
resize(width, height);          // compliant
```

Only functions declared in the same file (function declarations and `const` function values) are
checked, arguments count when they are names or property reads (`size.width`), and names shorter
than three letters (`a`, `b`, `x`, `y`) and symmetric pairs (`left`/`right`, `first`/`second`,
`from`/`to`, …) are left alone, since swapping those is often deliberate (a reversed comparison).
Names that differ only in a digit (`seq1`/`seq2`), calls that cross several pairs at once (a
deliberate mirror of the whole call), and recursive calls are not checked either.
"#,
    ..RuleMeta::new(
        "js/swapped-arguments",
        "js",
        "Arguments should not be passed in the positions of each other's parameters",
    )
};

/// Pairs whose swap is usually deliberate (reversing a comparison or a direction).
const SYMMETRIC: &[(&str, &str)] = &[
    ("left", "right"),
    ("lhs", "rhs"),
    ("first", "second"),
    ("prev", "next"),
    ("previous", "next"),
    ("from", "to"),
    ("source", "target"),
    ("src", "dst"),
    ("src", "dest"),
    ("old", "new"),
    ("before", "after"),
    ("start", "end"),
    ("one", "two"),
    ("this", "other"),
    ("self", "other"),
    ("actual", "expected"),
];

impl JsRule for SwappedArguments {
    fn meta(&self) -> &'static RuleMeta {
        &SWAPPED_ARGUMENTS
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CallExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::CallExpression(call) = node.kind() else {
            return;
        };
        if call.arguments.len() < 2 {
            return;
        }
        let Expression::Identifier(callee) = helpers::unwrap_expr(&call.callee) else {
            return;
        };
        // Count before collecting: most calls pass fewer than two plain names.
        if call
            .arguments
            .iter()
            .filter(|a| argument_name(a).is_some())
            .nth(1)
            .is_none()
        {
            return;
        }
        let names: Vec<Option<&str>> = call.arguments.iter().map(argument_name).collect();
        let Some((params, function_span)) = local_function_params(ctx, callee) else {
            return;
        };
        // Recursion reorders the function's own parameters on purpose.
        let at = call.span.start;
        if function_span.start <= at && at < function_span.end {
            return;
        }
        let mut swaps: Vec<(usize, usize, &str, &str)> = Vec::new();
        for i in 0..names.len() {
            for j in i + 1..names.len() {
                let (Some(a), Some(b)) = (names[i], names[j]) else {
                    continue;
                };
                let (Some(&pi), Some(&pj)) = (params.get(i), params.get(j)) else {
                    continue;
                };
                let (Some(pi), Some(pj)) = (pi, pj) else {
                    continue;
                };
                if a == pj && b == pi && pi != pj {
                    swaps.push((i, j, a, b));
                }
            }
        }
        // Several pairs crossed together is a deliberate mirror of the whole call
        // (`f(diff.swap(), seq2, seq1, range2, range1)`).
        let [(i, j, a, b)] = swaps.as_slice() else {
            return;
        };
        let (i, j, a, b) = (*i, *j, *a, *b);
        if a.len() < 3 || b.len() < 3 || symmetric(a, b) {
            return;
        }
        let (Some(first), Some(second)) = (call.arguments.get(i), call.arguments.get(j)) else {
            return;
        };
        // Checked last: linear in the number of uses of the function.
        if ctx
            .reference_symbol(callee)
            .is_none_or(|s| ctx.references_of(s).any(|r| r.is_write()))
        {
            return;
        }
        ctx.emit(
            Span::new(first.span().start, second.span().end),
            format!(
                "'{a}' and '{b}' are passed in each other's positions: '{}' declares '{b}' before '{a}'",
                callee.name
            ),
        );
    }
}

/// Names whose roles are interchangeable, so that swapping them is often deliberate: a known
/// pair (`left`/`right`, `from`/`to`, …) or names that differ only in digits (`seq1`/`seq2`).
fn symmetric(a: &str, b: &str) -> bool {
    let strip = |s: &str| {
        s.chars()
            .filter(|c| !c.is_ascii_digit())
            .collect::<String>()
    };
    if strip(a) == strip(b) {
        return true;
    }
    let (a, b) = (a.to_ascii_lowercase(), b.to_ascii_lowercase());
    SYMMETRIC
        .iter()
        .any(|&(x, y)| (a.contains(x) && b.contains(y)) || (a.contains(y) && b.contains(x)))
}

/// The name an argument carries: `width` or `size.width`.
fn argument_name<'a>(arg: &Argument<'a>) -> Option<&'a str> {
    match helpers::unwrap_expr(arg.as_expression()?) {
        Expression::Identifier(id) => Some(id.name.as_str()),
        Expression::StaticMemberExpression(m) => Some(m.property.name.as_str()),
        _ => None,
    }
}

/// Parameter names (`None` for destructured ones) of a function declared in this file and never
/// reassigned, with the function's span. Stops at a rest parameter.
fn local_function_params<'a>(
    ctx: &Ctx<'a>,
    callee: &IdentifierReference<'_>,
) -> Option<(Vec<Option<&'a str>>, Span)> {
    let symbol = ctx.reference_symbol(callee)?;
    if !ctx.scoping().symbol_redeclarations(symbol).is_empty() {
        return None;
    }
    let (params, span): (&'a FormalParameters<'a>, Span) =
        match ctx.symbol_declaration(symbol).kind() {
            AstKind::Function(f) if f.body.is_some() => (&f.params, f.span),
            AstKind::VariableDeclarator(d) if d.kind == VariableDeclarationKind::Const => {
                match d.init.as_ref().map(helpers::unwrap_expr)? {
                    Expression::ArrowFunctionExpression(a) => (&a.params, a.span),
                    Expression::FunctionExpression(f) => (&f.params, f.span),
                    _ => return None,
                }
            }
            _ => return None,
        };
    let names = params
        .items
        .iter()
        .map(|p| match &p.pattern {
            BindingPattern::BindingIdentifier(id) => Some(id.name.as_str()),
            BindingPattern::AssignmentPattern(a) => match &a.left {
                BindingPattern::BindingIdentifier(id) => Some(id.name.as_str()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    Some((names, span))
}
