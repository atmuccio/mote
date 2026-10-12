//! `java/magic-number` — unexplained numeric literals (off by default).

use super::util::Mods;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    default_enabled: false,
    tags: &["readability", "conventions"],
    params: &[ParamSpec {
        name: "ignore",
        default: ParamDefault::Str("-1,0,1,2"),
        description: "Comma-separated numbers that are never reported (compared by value, so `1` also accepts `1L` and `1.0`).",
    }],
    remediation_minutes: 5,
    description: r#"Detects numeric literals ("magic numbers") used directly in code instead of
through a named constant.

**Why it matters.** A bare `86400` or `0.075` says nothing about what it means, and when the
same value appears in several places, changing it means finding every copy — and not
changing an unrelated literal that happens to have the same value.

**How to fix.** Introduce a `static final` constant (or an enum) whose name explains the
value.

```java
// Noncompliant
if (retries > 5) { sleep(86400); }

// Compliant
private static final int MAX_RETRIES = 5;
private static final int SECONDS_PER_DAY = 86_400;
if (retries > MAX_RETRIES) { sleep(SECONDS_PER_DAY); }
```

Not reported: the numbers listed in `ignore` (default `-1, 0, 1, 2`), literals in initializers
of `static final` fields and interface constants, in static initializer blocks, and in private
static helpers that only build the value of such a field (`static final Map<..> CODES =
buildCodes();`), in data tables (an array initializer with three or more elements, or a row of
a nested one, such as `{161, "degree"}`, or a table written as calls: three or more
consecutive statements calling the same method on the same receiver with only literals,
constants and objects built from them as arguments, such as `codes.put(404, "Not Found");`;
a call that also takes a variable is an operation, not a row), in annotations, in enum constant arguments, in
`hashCode()` implementations, and in test files. Off by default: the rule is useful for
focused clean-ups but noisy on most code bases."#,
    ..RuleMeta::new("java/magic-number", "java", "Magic number")
};

#[derive(Default)]
pub struct MagicNumber {
    /// Parsed `ignore` parameter (lazily, on the first literal of the file).
    ignore: Option<Vec<f64>>,
    /// Methods already classified by `builds_constant`, by node id.
    builders: FxHashMap<usize, bool>,
    /// The last statement classified by `is_call_table_row` (node id, result).
    last_row: Option<(usize, bool)>,
    /// Answers of `in_exempt_context`'s walk up from an ancestor (see [`WalkKey`]): literals of
    /// deeply nested expressions share their ancestors, and walking up from each of them was
    /// quadratic in the depth.
    walks: FxHashMap<WalkKey, bool>,
}

/// The state of `in_exempt_context`'s walk on reaching an ancestor, which decides the rest of
/// the walk: the ancestor's node id, whether a statement was passed below it, and whether the
/// walk came up through an argument list.
type WalkKey = (usize, bool, bool);

/// Numeric value of a literal token (`1_000L`, `0x1F`, `017`, `0b11`, `2.5e3f`), without
/// allocating unless the literal contains underscores. `None` when it can't be represented.
fn literal_value(kind_name: &str, text: &str) -> Option<f64> {
    let owned;
    let text = if text.contains('_') {
        owned = text.replace('_', "");
        owned.as_str()
    } else {
        text
    };
    match kind_name {
        "decimal_floating_point_literal" => text
            .trim_end_matches(['f', 'F', 'd', 'D'])
            .parse::<f64>()
            .ok(),
        "decimal_integer_literal"
        | "hex_integer_literal"
        | "octal_integer_literal"
        | "binary_integer_literal" => {
            let t = text.trim_end_matches(['l', 'L']);
            let (digits, radix) = match kind_name {
                "hex_integer_literal" => (t.get(2..)?, 16),
                "binary_integer_literal" => (t.get(2..)?, 2),
                "octal_integer_literal" => (t.get(1..)?, 8),
                _ => (t, 10),
            };
            u64::from_str_radix(digits, radix).ok().map(|v| v as f64)
        }
        _ => None,
    }
}

/// An array initializer that holds data: three or more elements, or a row of a nested table.
fn is_data_table<'t>(ctx: &Ctx<'t>, init: Node<'t>) -> bool {
    named_children(init).nth(2).is_some()
        || ctx
            .parent(init)
            .is_some_and(|p| kind(p) == "array_initializer")
}

/// `MAX_SIZE`, `HOUR_OF_DAY`: a name written like a constant.
fn is_constant_name(name: &str) -> bool {
    name.bytes().any(|b| b.is_ascii_uppercase())
        && name
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
}

/// Whether `args` holds only data: literals, constants (`Calendar.HOUR_OF_DAY`, `MAX`),
/// objects built from them (`new Car(4, "Entry_4", 2500d)`, `LocalDate.of(2016, 2, 24)`, a
/// builder chain) — no variable, which would make the call an operation on state rather than
/// a row of a table.
fn is_data_only<'t>(ctx: &Ctx<'t>, args: Node<'t>) -> bool {
    let mut stack: Vec<(Node<'t>, Node<'t>)> = named_children(args).map(|c| (c, args)).collect();
    let mut steps = 0;
    while let Some((n, parent)) = stack.pop() {
        steps += 1;
        if steps > 400 {
            return false;
        }
        match kind(n) {
            "lambda_expression" | "class_body" | "method_reference" | "this" | "super" => {
                return false;
            }
            // A qualified constant (`java.util.Calendar.HOUR_OF_DAY`) or a qualified type a
            // call is made on (`java.time.LocalDate.of(..)`).
            "field_access"
                if ctx
                    .text(n)
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'$' || b == b'.') =>
            {
                let text = ctx.text(n);
                let last = text.rsplit('.').next().unwrap_or(text);
                let type_qualifier = kind(parent) == "method_invocation"
                    && parent
                        .field(Field::Object)
                        .is_some_and(|o| o.id() == n.id())
                    && last.starts_with(|c: char| c.is_ascii_uppercase());
                if !is_constant_name(last) && !type_qualifier {
                    stack.extend(named_children(n).map(|c| (c, n)));
                }
            }
            "identifier" => {
                let is = |f: Field| parent.field(f).is_some_and(|x| x.id() == n.id());
                let allowed = match kind(parent) {
                    // A method name, or a type the call is made on (`LocalDate.of(..)`).
                    "method_invocation" => {
                        is(Field::Name)
                            || (is(Field::Object)
                                && ctx.text(n).starts_with(|c: char| c.is_ascii_uppercase()))
                    }
                    // `Calendar.HOUR_OF_DAY`, `Continent.ASIA`.
                    "field_access" => {
                        is(Field::Field)
                            || ctx.text(n).starts_with(|c: char| c.is_ascii_uppercase())
                    }
                    _ => is_constant_name(ctx.text(n)),
                };
                if !allowed {
                    return false;
                }
            }
            _ => stack.extend(named_children(n).map(|c| (c, n))),
        }
    }
    true
}

/// The call of an expression statement that is a possible table row
/// (`cars.add(new Car(4, "Entry_4", 2500d));`) as (receiver text, method name): its arguments
/// hold only data.
fn statement_call<'t>(ctx: &Ctx<'t>, stmt: Node<'t>) -> Option<(&'t str, &'t str)> {
    if kind(stmt) != "expression_statement" {
        return None;
    }
    let call = syntax::first_named_child(stmt).filter(|c| kind(*c) == "method_invocation")?;
    let name = ctx.call_name(call)?;
    let recv = call.field(Field::Object).map_or("", |o| ctx.text(o));
    let args = call.field(Field::Arguments)?;
    is_data_only(ctx, args).then_some((recv, name))
}

/// A statement that is one row of a data table written as calls: at least three consecutive
/// statements call the same method on the same receiver with data only as arguments
/// (`codes.put(404, "Not Found");`, `cars.add(new Car(4, "Entry_4", 2500d));`), like the rows
/// of an array initializer. A call taking a variable (`out.writeLong(size, 8, 16)`) is an
/// operation, not a row.
fn is_call_table_row<'t>(ctx: &Ctx<'t>, stmt: Node<'t>) -> bool {
    let Some(shape) = statement_call(ctx, stmt) else {
        return false;
    };
    let same = |s: Node<'t>| statement_call(ctx, s) == Some(shape);
    let is_comment = |s: Node<'t>| matches!(kind(s), "line_comment" | "block_comment");
    let mut run = 1;
    let mut cur = stmt;
    while run < 3 {
        match ctx.prev_named_sibling(cur) {
            Some(p) if is_comment(p) => cur = p,
            Some(p) if same(p) => {
                run += 1;
                cur = p;
            }
            _ => break,
        }
    }
    cur = stmt;
    while run < 3 {
        match ctx.next_named_sibling(cur) {
            Some(n) if is_comment(n) => cur = n,
            Some(n) if same(n) => {
                run += 1;
                cur = n;
            }
            _ => break,
        }
    }
    run >= 3
}

/// A private static method without parameters that the file only calls to initialize a
/// `static final` field (`static final Map<Integer, String> CODES = buildCodes();`): it builds
/// a constant, like an initializer.
fn builds_constant<'t>(ctx: &Ctx<'t>, method: Node<'t>) -> bool {
    let mods = Mods::of(method);
    if !mods.has("private") || !mods.has("static") || syntax::parameters(method).next().is_some() {
        return false;
    }
    let name = syntax::name_of(method, ctx.source());
    let Some(body) = ctx.parent(method) else {
        return false;
    };
    // The name appears twice in the file: its declaration and the one initializer.
    let src = ctx.source();
    let b = src.as_bytes();
    let is_word = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
    let uses = src
        .match_indices(name)
        .filter(|(i, _)| {
            let end = i + name.len();
            let starts_word = *i == 0 || !is_word(b[i - 1]);
            starts_word && !b.get(end).is_some_and(|c| is_word(*c))
        })
        .count();
    uses == 2
        && ctx.members(body).any(|m| {
            kind(m) == "field_declaration"
                && Mods::of(m).has("static")
                && Mods::of(m).has("final")
                && syntax::declarators(m).any(|d| {
                    d.field(Field::Value).is_some_and(|v| {
                        kind(v) == "method_invocation"
                            && v.field(Field::Object).is_none()
                            && ctx.call_name(v) == Some(name)
                    })
                })
        })
}

/// Whether the literal sits in a context where numbers are self-explanatory or already named.
/// The walk stops at the enclosing member: nothing above it can exempt the literal.
fn in_exempt_context<'t>(
    node: Node<'t>,
    ctx: &Ctx<'t>,
    memo: &mut FxHashMap<usize, bool>,
    last_row: &mut Option<(usize, bool)>,
    walks: &mut FxHashMap<WalkKey, bool>,
) -> bool {
    let mut passed: Vec<WalkKey> = Vec::new();
    let answer = exempt_walk(node, ctx, memo, last_row, walks, &mut passed);
    for key in passed {
        walks.insert(key, answer);
    }
    answer
}

/// The walk up of [`in_exempt_context`]; records the states it passes in `passed`.
fn exempt_walk<'t>(
    node: Node<'t>,
    ctx: &Ctx<'t>,
    memo: &mut FxHashMap<usize, bool>,
    last_row: &mut Option<(usize, bool)>,
    walks: &FxHashMap<WalkKey, bool>,
    passed: &mut Vec<WalkKey>,
) -> bool {
    let mut prev = node;
    let mut in_statement = false;
    for a in ctx.ancestors(node) {
        let key = (a.id(), in_statement, kind(prev) == "argument_list");
        if let Some(&answer) = walks.get(&key) {
            return answer;
        }
        passed.push(key);
        match kind(a) {
            "static_initializer" => return true,
            // The innermost statement only: is it a row of a table written as calls?
            "expression_statement" if !in_statement => {
                in_statement = true;
                let row = match *last_row {
                    Some((id, known)) if id == a.id() => known,
                    _ => {
                        let known = is_call_table_row(ctx, a);
                        *last_row = Some((a.id(), known));
                        known
                    }
                };
                if row {
                    return true;
                }
            }
            "array_initializer" => {
                if is_data_table(ctx, a) {
                    return true;
                }
            }
            "annotation"
            | "annotation_argument_list"
            | "element_value_pair"
            | "annotation_type_element_declaration"
            | "constant_declaration" => return true,
            "enum_constant" => {
                if kind(prev) == "argument_list" {
                    return true;
                }
            }
            "field_declaration" => {
                return Mods::of(a).has("static") && Mods::of(a).has("final");
            }
            "method_declaration" => {
                if syntax::name_of(a, ctx.source()) == "hashCode"
                    && syntax::parameters(a).next().is_none()
                {
                    return true;
                }
                if let Some(&known) = memo.get(&a.id()) {
                    return known;
                }
                let builds = builds_constant(ctx, a);
                memo.insert(a.id(), builds);
                return builds;
            }
            "constructor_declaration"
            | "class_body"
            | "enum_body"
            | "interface_body"
            | "program" => return false,
            _ => {}
        }
        prev = a;
    }
    false
}

impl<'t> JavaRule<'t> for MagicNumber {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "decimal_integer_literal",
            "hex_integer_literal",
            "octal_integer_literal",
            "binary_integer_literal",
            "decimal_floating_point_literal",
            "hex_floating_point_literal",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if ctx.is_test() {
            return;
        }
        let mut value = literal_value(kind(node), ctx.text(node));
        let mut at = node;
        if let Some(p) = ctx.parent(node) {
            if kind(p) == "unary_expression" && syntax::operator_str(p) == "-" {
                value = value.map(|v| -v);
                at = p;
            }
        }
        let ignore = self.ignore.get_or_insert_with(|| {
            ctx.param_str("ignore")
                .split(',')
                .filter_map(|s| s.trim().parse::<f64>().ok())
                .collect()
        });
        if value.is_some_and(|v| ignore.contains(&v)) {
            return;
        }
        if in_exempt_context(
            node,
            ctx,
            &mut self.builders,
            &mut self.last_row,
            &mut self.walks,
        ) {
            return;
        }
        ctx.emit(
            at,
            format!(
                "Replace the magic number {} with a named constant",
                ctx.text(at)
            ),
        );
    }
}
