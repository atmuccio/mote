//! `js/prefer-number-isnan`

use crate::prelude::*;
use crate::rules::batch_6::util::{identifier_kind, ValueKind};

pub struct PreferNumberIsnan;

pub static PREFER_NUMBER_ISNAN: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["suspicious", "numbers"],
    remediation_minutes: 5,
    description: r#"Flags calls to the global `isNaN()` and `isFinite()` whose argument may not be a number.

**Why:** both functions convert their argument to a number before testing it, and the conversion
has surprising results: `isNaN('')`, `isNaN(' ')` and `isNaN(null)` are `false` (they convert to
`0`), `isFinite('')` and `isFinite(null)` are `true`, and `isNaN('12px')` is `true` while
`parseFloat('12px')` is `12`. Code that uses them to check whether user input "is a number"
therefore accepts empty and blank strings. `Number.isNaN()` and `Number.isFinite()` never
convert: they answer for numbers only.

**Fix:** convert explicitly first and test the result (`Number.isNaN(parseFloat(text))`, or a
regular expression for the accepted format), or call `Number.isNaN(x)` / `Number.isFinite(x)` when
`x` is already a number. For a date, test `Number.isNaN(date.getTime())`.

```js
if (!isNaN(input.value)) { save(input.value); }       // flagged: '' passes
if (isFinite(event.key)) { digit(event.key); }        // flagged: ' ' passes

const n = Number.parseFloat(input.value);
if (!Number.isNaN(n)) { save(n); }                    // compliant
```

Not reported: arguments that are certainly numbers, where the global functions give the same
answer (`isNaN(parseInt(s, 10))`, `isNaN(a / b)`, `isNaN(date.getTime())`, a `number` variable, a
variable whose last assignment before the call stores such a value, the result of a helper named
like `parseLocalizedInt` or `toNumber`), dates (which convert to their time value), and calls
guarded by `typeof x === 'number'`, `x instanceof Date` or a regular expression that checks the
text first (`/^\d+$/.test(x) && isFinite(x)`). In TypeScript, whose declarations only accept numbers,
a call is reported only when the argument is visibly something else: a string, or a value cast to
`any`, `unknown` or `string` to get past the type check.
"#,
    ..RuleMeta::new(
        "js/prefer-number-isnan",
        "js",
        "The global 'isNaN' and 'isFinite' should not be used on values that may not be numbers",
    )
};

/// What the check knows about the call it looks at.
#[derive(Clone, Copy)]
struct Probe {
    /// Start of the `isNaN(…)` call: the last write before it gives a variable its value.
    at: u32,
    /// `isNaN(null)` is `false` like `Number.isNaN(null)`, so `null` is harmless for `isNaN` (not
    /// for `isFinite`, where `isFinite(null)` is `true`).
    null_ok: bool,
}

/// `parseLocalizedInt`, `parseFloatValue`, `toNumber`: helpers whose name says they return a
/// number.
fn numeric_name(name: &str) -> bool {
    (name.starts_with("parse")
        && ["Int", "Integer", "Float", "Number"]
            .iter()
            .any(|suffix| name.ends_with(suffix)))
        || matches!(
            name,
            "toNumber" | "toInt" | "toInteger" | "toFloat" | "toFinite" | "toSafeInteger"
        )
}

/// `Math.*`, `Number(…)`, `parseInt(…)`, `date.getTime()`, … : calls that return a number.
fn numeric_call(ctx: &Ctx<'_>, call: &CallExpression<'_>) -> bool {
    let callee = helpers::unwrap_expr(&call.callee);
    if let Expression::Identifier(id) = callee {
        let name = id.name.as_str();
        return (matches!(name, "Number" | "parseInt" | "parseFloat")
            && ctx.is_global_reference(id))
            || numeric_name(name);
    }
    let Expression::StaticMemberExpression(member) = callee else {
        return false;
    };
    if let Expression::Identifier(object) = helpers::unwrap_expr(&member.object) {
        match (object.name.as_str(), member.property.name.as_str()) {
            ("Math", _)
            | ("Number", "parseInt" | "parseFloat")
            | ("Date", "now" | "parse" | "UTC") => {
                return ctx.is_global_reference(object);
            }
            _ => {}
        }
    }
    let name = member.property.name.as_str();
    numeric_name(name)
        || matches!(
            name,
            "getTime"
                | "getFullYear"
                | "getMonth"
                | "getDate"
                | "getDay"
                | "getHours"
                | "getMinutes"
                | "getSeconds"
                | "getMilliseconds"
                | "getUTCFullYear"
                | "getUTCMonth"
                | "getUTCDate"
                | "getUTCHours"
                | "getUTCMinutes"
                | "getUTCSeconds"
                | "getTimezoneOffset"
                | "charCodeAt"
                | "indexOf"
                | "lastIndexOf"
                | "findIndex"
                | "findLastIndex"
                | "localeCompare"
        )
}

/// Is `expr` certainly a number or a date, so that the conversion holds no surprise?
fn certainly_number(ctx: &Ctx<'_>, expr: &Expression<'_>, probe: Probe, depth: u8) -> bool {
    if depth > 8 {
        return false;
    }
    match helpers::unwrap_expr(expr) {
        Expression::NumericLiteral(_) | Expression::UpdateExpression(_) => true,
        Expression::NullLiteral(_) => probe.null_ok,
        Expression::UnaryExpression(u) => matches!(
            u.operator,
            UnaryOperator::UnaryNegation | UnaryOperator::UnaryPlus | UnaryOperator::BitwiseNot
        ),
        Expression::BinaryExpression(b) => match b.operator {
            // `n + x` is a number, or a string holding the digits of `n`: never a blank string.
            BinaryOperator::Addition => {
                certainly_number(ctx, &b.left, probe, depth + 1)
                    || certainly_number(ctx, &b.right, probe, depth + 1)
            }
            op => op.is_arithmetic() || op.is_bitwise(),
        },
        Expression::ConditionalExpression(c) => {
            certainly_number(ctx, &c.consequent, probe, depth + 1)
                && certainly_number(ctx, &c.alternate, probe, depth + 1)
        }
        Expression::CallExpression(call) => numeric_call(ctx, call),
        // A date converts to its time value: no string surprise.
        Expression::NewExpression(new) => is_global_date(ctx, &new.callee),
        Expression::StaticMemberExpression(m) => m.property.name == "length",
        Expression::Identifier(id) => {
            if matches!(id.name.as_str(), "NaN" | "Infinity") && ctx.is_global_reference(id) {
                return true;
            }
            number_variable(ctx, id, probe, depth)
        }
        _ => false,
    }
}

fn is_global_date(ctx: &Ctx<'_>, callee: &Expression<'_>) -> bool {
    matches!(helpers::unwrap_expr(callee), Expression::Identifier(id) if id.name == "Date" && ctx.is_global_reference(id))
}

/// The value a write gives a variable.
enum Written<'b, 'a> {
    Value(&'b Expression<'a>),
    Number,
    Unknown,
}

/// A variable or parameter declared as `number`, or whose last write before the call (its
/// initializer, `x = …`, `x++`, `x *= …`) stores a number.
fn number_variable(ctx: &Ctx<'_>, id: &IdentifierReference<'_>, probe: Probe, depth: u8) -> bool {
    let Some(symbol) = ctx.reference_symbol(id) else {
        return false;
    };
    let annotated_number = |t: Option<&TSTypeAnnotation<'_>>| {
        t.is_some_and(|t| matches!(t.type_annotation, TSType::TSNumberKeyword(_)))
    };
    let mut last: Option<(u32, Written<'_, '_>)> = None;
    match ctx.symbol_declaration(symbol).kind() {
        AstKind::VariableDeclarator(d) => {
            if !matches!(d.id, BindingPattern::BindingIdentifier(_)) {
                return false;
            }
            if annotated_number(d.type_annotation.as_deref()) {
                return true;
            }
            if let Some(init) = &d.init {
                last = Some((d.span.start, Written::Value(init)));
            }
        }
        AstKind::FormalParameter(p) => {
            if annotated_number(p.type_annotation.as_deref()) {
                return true;
            }
        }
        _ => return false,
    }
    for reference in ctx.references_of(symbol) {
        if !reference.is_write() {
            continue;
        }
        let node = ctx.node(reference.node_id());
        let at = node.span().start;
        if at >= probe.at || last.as_ref().is_some_and(|(p, _)| *p > at) {
            continue;
        }
        let written = match ctx.parent_kind(node) {
            Some(AstKind::AssignmentExpression(a)) if a.left.span().start == at => match a.operator
            {
                AssignmentOperator::Assign => Written::Value(&a.right),
                AssignmentOperator::Addition => Written::Unknown,
                op if op.is_arithmetic() || op.is_bitwise() => Written::Number,
                _ => Written::Unknown,
            },
            Some(AstKind::UpdateExpression(_)) => Written::Number,
            _ => Written::Unknown,
        };
        last = Some((at, written));
    }
    match last {
        Some((_, Written::Number)) => true,
        Some((_, Written::Value(value))) => certainly_number(ctx, value, probe, depth + 1),
        _ => false,
    }
}

/// Visible evidence, in TypeScript, that the argument is not a number: a string, or a cast
/// to `any` / `unknown` / `string` (`value as any`, `x as unknown as number`).
fn not_number_in_typescript(ctx: &Ctx<'_>, expr: &Expression<'_>) -> bool {
    let mut current = expr;
    loop {
        match current {
            Expression::TSAsExpression(a) => {
                if loose_type(&a.type_annotation) {
                    return true;
                }
                current = &a.expression;
            }
            Expression::TSTypeAssertion(a) => {
                if loose_type(&a.type_annotation) {
                    return true;
                }
                current = &a.expression;
            }
            Expression::TSNonNullExpression(n) => current = &n.expression,
            Expression::TSSatisfiesExpression(s) => current = &s.expression,
            _ => break,
        }
    }
    string_valued(ctx, current)
}

fn loose_type(ty: &TSType<'_>) -> bool {
    matches!(
        ty,
        TSType::TSAnyKeyword(_) | TSType::TSUnknownKeyword(_) | TSType::TSStringKeyword(_)
    )
}

/// A string literal, a template, or a variable declared as (or initialized with) a string.
fn string_valued(ctx: &Ctx<'_>, expr: &Expression<'_>) -> bool {
    match helpers::unwrap_expr(expr) {
        Expression::StringLiteral(_) | Expression::TemplateLiteral(_) => true,
        Expression::Identifier(id) => identifier_kind(ctx, id) == Some(ValueKind::String),
        _ => false,
    }
}

/// `typeof x === 'number' && isNaN(x)`: an enclosing `&&` chain checks the type first.
fn guarded_by_typeof(ctx: &Ctx<'_>, node: &AstNode<'_>, name: &str) -> bool {
    let mut child = node.span();
    for ancestor in ctx.ancestors(node) {
        let AstKind::LogicalExpression(l) = ancestor.kind() else {
            // Only look through the expression the call is part of.
            if ancestor.kind().is_statement() || ancestor.kind().is_function_like() {
                return false;
            }
            child = ancestor.span();
            continue;
        };
        if l.operator == LogicalOperator::And
            && l.right.span().contains_inclusive(child)
            && checks_number_type(&l.left, name)
        {
            return true;
        }
        child = ancestor.span();
    }
    false
}

/// Does this condition (or one of its `&&` operands) test `typeof name === 'number'`, call an
/// `isNumber(name)` helper, check for a `Date` (`name instanceof Date`, `[object Date]`), or test the
/// text with a regular expression?
fn checks_number_type(expr: &Expression<'_>, name: &str) -> bool {
    match helpers::unwrap_expr(expr) {
        Expression::LogicalExpression(l) if l.operator == LogicalOperator::And => {
            checks_number_type(&l.left, name) || checks_number_type(&l.right, name)
        }
        Expression::BinaryExpression(b) if b.operator == BinaryOperator::Instanceof => {
            matches!(helpers::unwrap_expr(&b.left), Expression::Identifier(id) if id.name == name)
                && matches!(helpers::unwrap_expr(&b.right), Expression::Identifier(c) if matches!(c.name.as_str(), "Date" | "Number"))
        }
        Expression::BinaryExpression(b)
            if b.operator.is_equality()
                && [&b.left, &b.right].iter().any(|e| {
                    matches!(
                        helpers::string_value(e),
                        Some("[object Date]" | "[object Number]")
                    )
                }) =>
        {
            true
        }
        Expression::BinaryExpression(b) if b.operator.is_equality() => {
            let (typeof_side, literal) = if helpers::is_typeof(&b.left) {
                (&b.left, &b.right)
            } else {
                (&b.right, &b.left)
            };
            let Expression::UnaryExpression(u) = helpers::unwrap_expr(typeof_side) else {
                return false;
            };
            u.operator == UnaryOperator::Typeof
                && helpers::string_value(literal) == Some("number")
                && matches!(helpers::unwrap_expr(&u.argument), Expression::Identifier(id) if id.name == name)
        }
        Expression::CallExpression(call) => {
            let callee = helpers::unwrap_expr(&call.callee);
            let argument = call
                .arguments
                .first()
                .and_then(helpers::argument_expr)
                .map(helpers::unwrap_expr);
            match callee {
                Expression::Identifier(id) => {
                    id.name == "isNumber" && argument.is_some_and(|a| names(a, name))
                }
                Expression::StaticMemberExpression(m) => match m.property.name.as_str() {
                    "isNumber" => argument.is_some_and(|a| names(a, name)),
                    // `/^-?\d+$/.test(String(x))`: the format of the text is checked first.
                    "test" => {
                        matches!(
                            helpers::unwrap_expr(&m.object),
                            Expression::RegExpLiteral(_)
                        ) && argument.is_some_and(|a| names(a, name) || stringified(a, name))
                    }
                    _ => false,
                },
                _ => false,
            }
        }
        _ => false,
    }
}

/// Is `expr` the variable `name`?
fn names(expr: &Expression<'_>, name: &str) -> bool {
    matches!(helpers::unwrap_expr(expr), Expression::Identifier(id) if id.name == name)
}

/// `String(name)` or `name.toString()`.
fn stringified(expr: &Expression<'_>, name: &str) -> bool {
    let Expression::CallExpression(call) = helpers::unwrap_expr(expr) else {
        return false;
    };
    match helpers::unwrap_expr(&call.callee) {
        Expression::Identifier(id) if id.name == "String" => call
            .arguments
            .first()
            .and_then(helpers::argument_expr)
            .is_some_and(|a| names(a, name)),
        Expression::StaticMemberExpression(m) if m.property.name == "toString" => {
            names(&m.object, name)
        }
        _ => false,
    }
}

impl JsRule for PreferNumberIsnan {
    fn meta(&self) -> &'static RuleMeta {
        &PREFER_NUMBER_ISNAN
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CallExpression]
    }

    fn callee_names(&self) -> &'static [&'static str] {
        &["isNaN", "isFinite"]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::CallExpression(call) = node.kind() else {
            return;
        };
        let Expression::Identifier(callee) = helpers::unwrap_expr(&call.callee) else {
            return;
        };
        let name = callee.name.as_str();
        if name != "isNaN" && name != "isFinite" {
            return;
        }
        let [argument] = call.arguments.as_slice() else {
            return;
        };
        let Some(argument) = helpers::argument_expr(argument) else {
            return;
        };
        if !ctx.is_global_reference(callee) {
            return;
        }
        if ctx.is_typescript() {
            if !not_number_in_typescript(ctx, argument) {
                return;
            }
        } else if certainly_number(
            ctx,
            argument,
            Probe {
                at: call.span.start,
                null_ok: name == "isNaN",
            },
            0,
        ) {
            return;
        }
        if let Expression::Identifier(id) = helpers::unwrap_expr(argument) {
            if guarded_by_typeof(ctx, node, id.name.as_str()) {
                return;
            }
        }
        let message = if name == "isNaN" {
            "Use 'Number.isNaN()' on a number, or convert explicitly first: the global 'isNaN' converts its argument, so '' and ' ' count as numbers"
        } else {
            "Use 'Number.isFinite()' on a number, or convert explicitly first: the global 'isFinite' converts its argument, so '' and null count as finite"
        };
        ctx.emit(callee.span, message);
    }
}
