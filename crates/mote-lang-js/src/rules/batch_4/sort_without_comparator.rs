//! `js/sort-without-comparator`

use crate::prelude::*;

pub struct SortWithoutComparator;

pub static SORT_WITHOUT_COMPARATOR: RuleMeta = RuleMeta {
    severity: Severity::Medium,
    tags: &["suspicious"],
    remediation_minutes: 2,
    description: r#"Flags `.sort()` and `.toSorted()` called without a compare function on arrays that are known to
hold numbers.

**Why:** without a compare function, JavaScript converts the elements to strings and sorts them
alphabetically, so `[10, 9, 1].sort()` gives `[1, 10, 9]`. Code that sorts numbers this way
works for small single-digit test data and fails on real input.

**Fix:** pass a numeric compare function: `(a, b) => a - b` for ascending order, `(a, b) => b - a`
for descending.

```ts
const sizes: number[] = getSizes();
sizes.sort();                          // flagged: sorted as strings
[3, 20, 100].toSorted();               // flagged
ids.map(Number).sort();                // flagged

sizes.sort((a, b) => a - b);           // compliant
names.sort();                          // compliant: not known to hold numbers
new Float64Array(values).sort();       // compliant: typed arrays sort numerically
```

The array must be recognisably numeric: an array literal of numbers, a variable declared with a
`number[]` / `Array<number>` type or initialised with such a literal (and never reassigned), or the
result of `.map(Number)`, `.map(parseInt)`, or `.map(parseFloat)`.
"#,
    ..RuleMeta::new(
        "js/sort-without-comparator",
        "js",
        "Numeric arrays should be sorted with a compare function",
    )
};

impl JsRule for SortWithoutComparator {
    fn meta(&self) -> &'static RuleMeta {
        &SORT_WITHOUT_COMPARATOR
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CallExpression]
    }

    fn callee_names(&self) -> &'static [&'static str] {
        &["sort", "toSorted"]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::CallExpression(call) = node.kind() else {
            return;
        };
        if !call.arguments.is_empty() {
            return;
        }
        let Expression::StaticMemberExpression(member) = helpers::unwrap_expr(&call.callee) else {
            return;
        };
        let method = member.property.name.as_str();
        if method != "sort" && method != "toSorted" {
            return;
        }
        if !is_numeric_array(ctx, &member.object) {
            return;
        }
        ctx.emit(
            member.property.span,
            format!("Pass a compare function such as '(a, b) => a - b' to '{method}': without one, numbers are sorted as strings"),
        );
    }
}

/// Is the expression recognisably an array of numbers?
fn is_numeric_array(ctx: &Ctx<'_>, expr: &Expression<'_>) -> bool {
    match helpers::unwrap_expr(expr) {
        // Single digits sort the same as text and as numbers: that literal is sorted correctly.
        Expression::ArrayExpression(array) => {
            is_numeric_literal_array(array) && !all_single_digits(array)
        }
        Expression::CallExpression(call) => is_map_to_number(ctx, call),
        Expression::Identifier(id) => {
            let Some(symbol) = ctx.reference_symbol(id) else {
                return false;
            };
            numeric_variable(ctx, symbol)
        }
        _ => false,
    }
}

/// `[1, 2, -3]`: a non-empty array literal of number literals only.
fn is_numeric_literal_array(array: &ArrayExpression<'_>) -> bool {
    !array.elements.is_empty()
        && array.elements.iter().all(|e| {
            e.as_expression()
                .is_some_and(|e| helpers::number_value(e).is_some())
        })
}

fn all_single_digits(array: &ArrayExpression<'_>) -> bool {
    array.elements.iter().all(|e| {
        e.as_expression()
            .and_then(helpers::number_value)
            .is_some_and(|v| (0.0..10.0).contains(&v) && v.fract() == 0.0)
    })
}

/// `xs.map(Number)`, `xs.map(parseFloat)`, `xs.map(parseInt)` with the global functions.
fn is_map_to_number(ctx: &Ctx<'_>, call: &CallExpression<'_>) -> bool {
    let Expression::StaticMemberExpression(member) = helpers::unwrap_expr(&call.callee) else {
        return false;
    };
    if member.property.name != "map" || call.arguments.len() != 1 {
        return false;
    }
    let Some(Expression::Identifier(f)) = call.arguments.first().and_then(Argument::as_expression)
    else {
        return false;
    };
    // `parseInt` as a map callback receives the index as its radix (`['1','2'].map(parseInt)` is
    // `[1, NaN]`) — still numbers, and still sorted as strings.
    matches!(f.name.as_str(), "Number" | "parseFloat" | "parseInt") && ctx.is_global_reference(f)
}

/// A variable whose declaration says it holds numbers and that is never reassigned.
fn numeric_variable(ctx: &Ctx<'_>, symbol: SymbolId) -> bool {
    if !ctx.scoping().symbol_redeclarations(symbol).is_empty()
        || ctx.references_of(symbol).any(|r| r.is_write())
    {
        return false;
    }
    let decl = ctx.symbol_declaration(symbol);
    match decl.kind() {
        AstKind::VariableDeclarator(d) => {
            let BindingPattern::BindingIdentifier(_) = &d.id else {
                return false;
            };
            if let Some(annotation) = &d.type_annotation {
                return is_number_array_type(&annotation.type_annotation);
            }
            match d.init.as_ref().map(helpers::unwrap_expr) {
                Some(Expression::ArrayExpression(array)) => is_numeric_literal_array(array),
                Some(Expression::CallExpression(call)) => is_map_to_number(ctx, call),
                _ => false,
            }
        }
        AstKind::FormalParameter(p) => {
            matches!(p.pattern, BindingPattern::BindingIdentifier(_))
                && p.type_annotation
                    .as_ref()
                    .is_some_and(|a| is_number_array_type(&a.type_annotation))
        }
        _ => false,
    }
}

/// `number[]`, `readonly number[]`, `Array<number>`, `ReadonlyArray<number>`.
fn is_number_array_type(ty: &TSType<'_>) -> bool {
    match ty {
        TSType::TSArrayType(a) => matches!(a.element_type, TSType::TSNumberKeyword(_)),
        TSType::TSTypeOperatorType(op) => {
            op.operator == TSTypeOperatorOperator::Readonly
                && is_number_array_type(&op.type_annotation)
        }
        TSType::TSTypeReference(r) => {
            let TSTypeName::IdentifierReference(name) = &r.type_name else {
                return false;
            };
            if name.name != "Array" && name.name != "ReadonlyArray" {
                return false;
            }
            r.type_arguments.as_ref().is_some_and(|args| {
                args.params.len() == 1
                    && matches!(args.params.first(), Some(TSType::TSNumberKeyword(_)))
            })
        }
        _ => false,
    }
}
