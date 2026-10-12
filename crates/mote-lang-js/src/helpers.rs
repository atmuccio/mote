//! Small, allocation-free AST helpers that rules commonly need.
//!
//! Oxc's AST types already offer many helpers — check them first:
//! `Expression::{is_null, is_undefined, is_void_0, is_literal, is_string_literal, get_inner_expression,
//! is_specific_id, is_specific_member_access, get_identifier_reference, is_function}`,
//! `CallExpression::callee_name`, `PropertyKey::{static_name, is_specific_static_name}`,
//! `BindingPattern::{get_binding_identifiers, get_identifier_name}`, `Statement::is_iteration_statement`,
//! and the operator helpers on `BinaryOperator` / `LogicalOperator` / `AssignmentOperator`.

use oxc_ast::ast::{
    Argument, BindingIdentifier, BindingPattern, CallExpression, ChainElement, Expression,
    PropertyKey, Statement, UnaryOperator,
};
use oxc_ast::AstKind;
use oxc_syntax::operator::BinaryOperator;
use rustc_hash::FxHashMap;
use std::borrow::Cow;

/// Strip parentheses and type-only wrappers (`as`, `satisfies`, `!`, `<T>x`, `f<T>`).
pub fn unwrap_expr<'b, 'a>(expr: &'b Expression<'a>) -> &'b Expression<'a> {
    expr.get_inner_expression()
}

/// Value of a string literal or a template literal without substitutions.
pub fn string_value<'a>(expr: &Expression<'a>) -> Option<&'a str> {
    match unwrap_expr(expr) {
        Expression::StringLiteral(s) => Some(s.value.as_str()),
        Expression::TemplateLiteral(t) if t.expressions.is_empty() => t
            .quasis
            .first()
            .and_then(|q| q.value.cooked)
            .map(|c| c.as_str()),
        _ => None,
    }
}

/// Numeric value of a number literal, optionally preceded by unary `-` / `+`.
pub fn number_value(expr: &Expression<'_>) -> Option<f64> {
    match unwrap_expr(expr) {
        Expression::NumericLiteral(n) => Some(n.value),
        Expression::UnaryExpression(u) => {
            let inner = number_value(&u.argument)?;
            match u.operator {
                UnaryOperator::UnaryNegation => Some(-inner),
                UnaryOperator::UnaryPlus => Some(inner),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Literal of a primitive value: string, number, bigint, boolean, `null`, regexp, or a template
/// literal without substitutions.
pub fn is_literal(expr: &Expression<'_>) -> bool {
    match unwrap_expr(expr) {
        Expression::StringLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BigIntLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::RegExpLiteral(_) => true,
        Expression::TemplateLiteral(t) => t.expressions.is_empty(),
        _ => false,
    }
}

/// `typeof x`.
pub fn is_typeof(expr: &Expression<'_>) -> bool {
    matches!(unwrap_expr(expr), Expression::UnaryExpression(u) if u.operator == UnaryOperator::Typeof)
}

/// The runtime property name of a key, normalized the way JavaScript converts keys to strings:
/// `a`, `'a'`, `"a"`, `` `a` `` and `['a']` are all `a`; `1`, `1.0`, `0x1` and `'1'` are all `1`.
/// `None` for computed non-literal keys, private names, and numbers whose string form we don't
/// reproduce exactly (very large/small magnitudes, bigints).
pub fn property_key_name<'a>(key: &PropertyKey<'a>) -> Option<Cow<'a, str>> {
    match key {
        PropertyKey::StaticIdentifier(id) => Some(Cow::Borrowed(id.name.as_str())),
        PropertyKey::PrivateIdentifier(_) => None,
        PropertyKey::StringLiteral(s) => Some(Cow::Borrowed(s.value.as_str())),
        PropertyKey::TemplateLiteral(t) if t.expressions.is_empty() => t
            .quasis
            .first()
            .and_then(|q| q.value.cooked)
            .map(|c| Cow::Borrowed(c.as_str())),
        PropertyKey::NumericLiteral(n) => {
            let v = n.value;
            // JavaScript prints plain decimals for 1e-6 <= |v| < 1e21, like Rust's `Display`.
            if v == 0.0 {
                Some(Cow::Borrowed("0"))
            } else if v.is_finite() && (1e-6..1e21).contains(&v.abs()) {
                Some(Cow::Owned(v.to_string()))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// `object.property` or `object?.property` where both are plain names, e.g. `("console", "log")`.
/// Also accepts `object["property"]` with a string literal.
pub fn static_member_names<'a>(expr: &Expression<'a>) -> Option<(&'a str, &'a str)> {
    let (object, property) = match unwrap_expr(expr) {
        Expression::StaticMemberExpression(m) => (&m.object, m.property.name.as_str()),
        Expression::ComputedMemberExpression(m) => (&m.object, string_value(&m.expression)?),
        _ => return None,
    };
    match unwrap_expr(object) {
        Expression::Identifier(id) => Some((id.name.as_str(), property)),
        _ => None,
    }
}

/// `object.method(...)` names of a call (see [`static_member_names`]).
pub fn call_member_names<'a>(call: &CallExpression<'a>) -> Option<(&'a str, &'a str)> {
    static_member_names(&call.callee)
}

/// The name a callee ends in, through parentheses, type-only wrappers and an optional chain: `f`
/// for `f()`, `m` for `a.b.m()`, `a?.m()`, `(a?.m)()`, `a['m']()` and `this.#m()` (the
/// private name without `#`); a computed key is read like
/// [`ComputedMemberExpression::static_property_name`](oxc_ast::ast::ComputedMemberExpression::static_property_name).
/// `None` for other callees (`f()()`, `a[k]()`, `(() => 1)()`). The plugin keys
/// [`JsRule::callee_names`](crate::JsRule::callee_names) on it.
pub fn callee_name<'a>(callee: &Expression<'a>) -> Option<&'a str> {
    match callee.get_inner_expression() {
        Expression::Identifier(id) => Some(id.name.as_str()),
        Expression::StaticMemberExpression(m) => Some(m.property.name.as_str()),
        Expression::ComputedMemberExpression(m) => m.static_property_name().map(|n| n.as_str()),
        Expression::PrivateFieldExpression(m) => Some(m.field.name.as_str()),
        Expression::ChainExpression(chain) => match &chain.expression {
            ChainElement::StaticMemberExpression(m) => Some(m.property.name.as_str()),
            ChainElement::ComputedMemberExpression(m) => {
                m.static_property_name().map(|n| n.as_str())
            }
            ChainElement::PrivateFieldExpression(m) => Some(m.field.name.as_str()),
            _ => None,
        },
        _ => None,
    }
}

/// Call arguments as expressions (`None` for a spread argument).
pub fn argument_expr<'b, 'a>(arg: &'b Argument<'a>) -> Option<&'b Expression<'a>> {
    arg.as_expression()
}

/// `==` or `!=`.
pub fn is_loose_equality(op: BinaryOperator) -> bool {
    matches!(op, BinaryOperator::Equality | BinaryOperator::Inequality)
}

/// `==`, `!=`, `===`, `!==`.
pub fn is_equality(op: BinaryOperator) -> bool {
    op.is_equality()
}

/// `==`, `!=`, `===`, `!==`, `<`, `<=`, `>`, `>=`.
pub fn is_comparison(op: BinaryOperator) -> bool {
    op.is_equality() || op.is_compare()
}

/// The operator that gives the same result with swapped operands (`a < b` ⇔ `b > a`).
pub fn mirror_comparison(op: BinaryOperator) -> Option<BinaryOperator> {
    use BinaryOperator::*;
    Some(match op {
        LessThan => GreaterThan,
        LessEqualThan => GreaterEqualThan,
        GreaterThan => LessThan,
        GreaterEqualThan => LessEqualThan,
        Equality | Inequality | StrictEquality | StrictInequality => op,
        _ => return None,
    })
}

/// The logical negation of an equality operator (`===` ⇔ `!==`). Relational operators are not
/// negatable in general because of `NaN`, so they return `None`.
pub fn negate_equality(op: BinaryOperator) -> Option<BinaryOperator> {
    use BinaryOperator::*;
    Some(match op {
        Equality => Inequality,
        Inequality => Equality,
        StrictEquality => StrictInequality,
        StrictInequality => StrictEquality,
        _ => return None,
    })
}

/// `for`, `for…in`, `for…of`, `while`, `do…while`.
pub fn is_loop(kind: AstKind<'_>) -> bool {
    kind.is_iteration_statement()
}

/// Is the statement a block with no statements (comments are not checked)?
pub fn is_empty_block(stmt: &Statement<'_>) -> bool {
    matches!(stmt, Statement::BlockStatement(b) if b.body.is_empty())
}

/// Visit every identifier bound by a pattern (`a`, `{ a, b: [c] }`, `...rest`, `x = 1`).
pub fn for_each_binding<'a>(
    pattern: &BindingPattern<'a>,
    f: &mut dyn FnMut(&BindingIdentifier<'a>),
) {
    match pattern {
        BindingPattern::BindingIdentifier(id) => f(id),
        BindingPattern::AssignmentPattern(p) => for_each_binding(&p.left, f),
        BindingPattern::ObjectPattern(p) => {
            for prop in &p.properties {
                for_each_binding(&prop.value, f);
            }
            if let Some(rest) = &p.rest {
                for_each_binding(&rest.argument, f);
            }
        }
        BindingPattern::ArrayPattern(p) => {
            for element in p.elements.iter().flatten() {
                for_each_binding(element, f);
            }
            if let Some(rest) = &p.rest {
                for_each_binding(&rest.argument, f);
            }
        }
    }
}

/// A set of names for scanning a whole file against. Each distinct name gets a dense index
/// (`0..len`, in order of first appearance) so callers can keep per-name flags in a `Vec`.
/// [`NameSet::get`] rejects most names that are not in the set with a 256-bit filter over
/// (length, first byte, last byte) before it hashes anything.
/// The 256-bit filter of [`NameSet`] alone: whether a name may be in a fixed set (no false
/// negatives), to reject most names before a hash lookup.
#[derive(Clone, Copy, Default)]
pub(crate) struct NameFilter([u64; 4]);

impl NameFilter {
    /// The filter of a fixed list, built at compile time (for a `const`/`static`).
    pub(crate) const fn of(names: &[&str]) -> Self {
        let mut bits = [0u64; 4];
        let mut i = 0;
        while i < names.len() {
            let slot = NameSet::slot(names[i]);
            bits[slot >> 6] |= 1u64 << (slot & 63);
            i += 1;
        }
        NameFilter(bits)
    }

    pub(crate) fn new<'n>(names: impl IntoIterator<Item = &'n str>) -> Self {
        let mut bits = [0u64; 4];
        for name in names {
            let slot = NameSet::slot(name);
            bits[slot >> 6] |= 1u64 << (slot & 63);
        }
        NameFilter(bits)
    }

    #[inline]
    pub(crate) fn may_contain(&self, name: &str) -> bool {
        let slot = NameSet::slot(name);
        self.0[slot >> 6] & (1u64 << (slot & 63)) != 0
    }
}

#[derive(Default)]
pub struct NameSet<'a> {
    index: FxHashMap<&'a str, usize>,
    filter: [u64; 4],
}

impl<'a> NameSet<'a> {
    pub fn new(names: impl IntoIterator<Item = &'a str>) -> Self {
        let mut out = NameSet::default();
        for name in names {
            let slot = Self::slot(name);
            out.filter[slot >> 6] |= 1u64 << (slot & 63);
            let next = out.index.len();
            out.index.entry(name).or_insert(next);
        }
        out
    }

    #[inline]
    const fn slot(name: &str) -> usize {
        let b = name.as_bytes();
        let (first, last) = match b {
            [] => (0, 0),
            [f, .., l] => (*f as usize, *l as usize),
            [f] => (*f as usize, *f as usize),
        };
        (b.len().wrapping_mul(37) ^ first.wrapping_mul(5) ^ last.wrapping_mul(11)) & 255
    }

    /// The index of `name`, if it is in the set.
    #[inline]
    pub fn get(&self, name: &str) -> Option<usize> {
        let slot = Self::slot(name);
        if self.filter[slot >> 6] & (1u64 << (slot & 63)) == 0 {
            return None;
        }
        self.index.get(name).copied()
    }

    /// Is `name` in the set?
    #[inline]
    pub fn contains(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// Number of distinct names.
    pub fn len(&self) -> usize {
        self.index.len()
    }

    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_set() {
        let names = ["", "a", "ab", "value", "_value", "#x"];
        let set = NameSet::new(names);
        for n in names {
            assert!(set.contains(n), "{n}");
        }
        for n in ["b", "abc", "values", "valuE", "x", "#y", "Value"] {
            assert!(!set.contains(n), "{n}");
        }
        assert!(NameSet::default().is_empty() && !set.is_empty());
        let dup = NameSet::new(["b", "a", "b"]);
        assert_eq!(
            (dup.len(), dup.get("b"), dup.get("a")),
            (2, Some(0), Some(1))
        );
    }

    #[test]
    fn comparison_helpers() {
        assert_eq!(
            mirror_comparison(BinaryOperator::LessThan),
            Some(BinaryOperator::GreaterThan)
        );
        assert_eq!(
            negate_equality(BinaryOperator::StrictEquality),
            Some(BinaryOperator::StrictInequality)
        );
        assert_eq!(negate_equality(BinaryOperator::LessThan), None);
        assert!(is_loose_equality(BinaryOperator::Inequality));
        assert!(!is_loose_equality(BinaryOperator::StrictInequality));
        assert!(is_comparison(BinaryOperator::GreaterEqualThan));
    }
}

/// Deepest nesting of groups and character classes that rules let the regular expression parser
/// see. The parser (and the rules' walkers) recurse once per level, so a deeply nested pattern —
/// a megabyte of `(`, or of `[` in `v` mode — would overflow the stack, which aborts the process.
pub const REGEX_MAX_NESTING: usize = 64;

/// Longest regular expression pattern rules parse (in bytes). Real patterns are far shorter; a
/// machine-made one of megabytes (four million alternatives) took gigabytes to parse.
pub const REGEX_MAX_LEN: usize = 32 << 10;

/// Whether a regular expression pattern (or the text of a string handed to `RegExp`) is at most
/// [`REGEX_MAX_LEN`] long and its groups and character classes nest at most `max` levels deep.
/// Every regex parse in the plugin is guarded by this check. Conservative: inside a class every
/// unescaped `[` counts as a nested class (as in `v` mode), and unbalanced brackets count as
/// open.
pub fn regex_within_limits(pattern: &str, max: usize) -> bool {
    if pattern.len() > REGEX_MAX_LEN {
        return false;
    }
    let mut depth = 0usize;
    let mut classes = 0usize;
    let mut escaped = false;
    for b in pattern.bytes() {
        if escaped {
            escaped = false;
            continue;
        }
        match b {
            b'\\' => escaped = true,
            b'[' => {
                classes += 1;
                depth += 1;
                if depth > max {
                    return false;
                }
            }
            b']' if classes > 0 => {
                classes -= 1;
                depth -= 1;
            }
            b'(' if classes == 0 => {
                depth += 1;
                if depth > max {
                    return false;
                }
            }
            b')' if classes == 0 => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    true
}

#[cfg(test)]
mod regex_nesting_tests {
    use super::*;

    #[test]
    fn counts_groups_and_classes() {
        assert!(regex_within_limits("a(b)[c]", 1));
        assert!(!regex_within_limits("((a))", 1));
        assert!(regex_within_limits("((a))", 2));
        // Nested classes (`v` mode) count.
        assert!(!regex_within_limits("[[a]]", 1));
        assert!(!regex_within_limits(&"[".repeat(100), 64));
        // Parentheses inside a class and escaped brackets do not.
        assert!(regex_within_limits("[((((]", 1));
        assert!(regex_within_limits(r"\(\(\(\[\[", 0));
        assert!(!regex_within_limits(
            &format!("{}a{}", "(".repeat(65), ")".repeat(65)),
            64
        ));
        assert!(regex_within_limits(
            &format!("{}a{}", "(".repeat(64), ")".repeat(64)),
            64
        ));
    }
}
