//! Where a constant regular expression is used: the regex-taking JDK calls
//! (`Pattern.compile`, `Pattern.matches`, `String.matches` / `replaceAll` / `replaceFirst` /
//! `split`) and the Bean Validation `@Pattern(regexp = …)` annotation. Shared by the regex
//! rules of batches 9 and 14.

use super::regex_syntax::{
    self as rx, Regex, CANON_EQ, CASE_INSENSITIVE, COMMENTS, DOTALL, LITERAL, MULTILINE,
    UNICODE_CASE, UNICODE_CHARACTER_CLASS, UNIX_LINES,
};
use super::util;
use crate::prelude::*;

/// Method names that can take a regular expression.
pub(crate) const CALL_NAMES: &[&str] =
    &["compile", "matches", "replaceAll", "replaceFirst", "split"];

/// How the pattern is applied to the input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Usage {
    /// The whole input must match (`matches`).
    Full,
    /// The pattern is searched for (`split`, `replaceAll`, `find`).
    Find,
    /// Not known (a compiled pattern used elsewhere).
    Unknown,
}

/// A constant regular expression and how it is used.
pub(crate) struct RegexUse<'t> {
    /// The pattern expression (reported).
    pub(crate) node: Node<'t>,
    pub(crate) pattern: String,
    pub(crate) flags: u32,
    pub(crate) usage: Usage,
    /// The call is `String.split` or `Pattern.split`.
    pub(crate) split: bool,
    /// The call is `replaceAll` / `replaceFirst` with an empty string literal as replacement.
    pub(crate) empty_replacement: bool,
}

impl RegexUse<'_> {
    /// Parses the pattern; `None` when it is outside what the parser models.
    pub(crate) fn parse(&self) -> Option<Regex> {
        rx::parse(&self.pattern, self.flags)
    }
}

/// The constant regular expression used by `node` (a `method_invocation` or an `annotation`),
/// when its text passes `wanted` (a cheap pre-filter run before any resolver work).
pub(crate) fn regex_use<'t>(
    ctx: &Ctx<'t>,
    node: Node<'t>,
    wanted: impl Fn(&str) -> bool,
) -> Option<RegexUse<'t>> {
    match kind(node) {
        "method_invocation" => from_call(ctx, node, &wanted),
        "annotation" => from_annotation(ctx, node, &wanted),
        _ => None,
    }
}

fn from_call<'t>(
    ctx: &Ctx<'t>,
    call: Node<'t>,
    wanted: &impl Fn(&str) -> bool,
) -> Option<RegexUse<'t>> {
    let name = ctx.call_name(call)?;
    let n = util::arg_count(call);
    let (is_static, usage) = match (name, n) {
        ("compile", 1 | 2) => (true, Usage::Unknown),
        ("matches", 2) => (true, Usage::Full),
        ("matches", 1) => (false, Usage::Full),
        ("replaceAll" | "replaceFirst", 2) | ("split", 1 | 2) => (false, Usage::Find),
        _ => return None,
    };
    let arg0 = util::arg(call, 0)?;
    if !matches!(
        kind(syntax::strip_parens(arg0)),
        "string_literal" | "binary_expression"
    ) {
        return None;
    }
    let recv = call.field(Field::Object)?;
    let pattern = util::string_constant(ctx, arg0)?;
    if !wanted(&pattern) {
        return None;
    }
    let flags = if name == "compile" && n == 2 {
        flag_value(ctx, util::arg(call, 1)?)?
    } else {
        0
    };
    if is_static {
        if !util::names_type(ctx, recv, "java.util.regex.Pattern") {
            return None;
        }
    } else if !util::receiver_is_string(ctx, call) {
        return None;
    }
    let usage = if name == "compile" {
        compiled_usage(ctx, call)
    } else {
        usage
    };
    let empty_replacement = matches!(name, "replaceAll" | "replaceFirst")
        && util::arg(call, 1)
            .map(syntax::strip_parens)
            .is_some_and(|r| kind(r) == "string_literal" && ctx.text(r) == "\"\"");
    Some(RegexUse {
        node: arg0,
        pattern,
        flags,
        usage,
        split: name == "split" || compiled_split(ctx, call),
        empty_replacement,
    })
}

/// The call on a freshly compiled pattern, `Pattern.compile(..).<name>(..)`, if any.
fn chained_call<'t>(ctx: &Ctx<'t>, call: Node<'t>) -> Option<Node<'t>> {
    let parent = ctx.parent(call)?;
    (kind(parent) == "method_invocation"
        && parent.field(Field::Object).map(|o| o.id()) == Some(call.id()))
    .then_some(parent)
}

fn compiled_split<'t>(ctx: &Ctx<'t>, call: Node<'t>) -> bool {
    ctx.call_name(call) == Some("compile")
        && chained_call(ctx, call)
            .is_some_and(|c| matches!(ctx.call_name(c), Some("split" | "splitAsStream")))
}

/// How `Pattern.compile(..)` is used when a call is chained right onto it.
fn compiled_usage<'t>(ctx: &Ctx<'t>, call: Node<'t>) -> Usage {
    let Some(next) = chained_call(ctx, call) else {
        return Usage::Unknown;
    };
    match ctx.call_name(next) {
        Some("split" | "splitAsStream" | "asPredicate") => Usage::Find,
        Some("asMatchPredicate") => Usage::Full,
        Some("matcher") => match chained_call(ctx, next).and_then(|c| ctx.call_name(c)) {
            Some("matches") => Usage::Full,
            Some("find" | "replaceAll" | "replaceFirst" | "results") => Usage::Find,
            _ => Usage::Unknown,
        },
        _ => Usage::Unknown,
    }
}

/// The value of a `Pattern.compile` flags argument made of `Pattern` constants joined with `|`
/// or `+` (or an integer literal); `None` for anything else.
fn flag_value<'t>(ctx: &Ctx<'t>, e: Node<'t>) -> Option<u32> {
    let e = syntax::strip_parens(e);
    match kind(e) {
        "binary_expression" => {
            if !matches!(syntax::operator_str(e), "|" | "+") {
                return None;
            }
            let l = flag_value(ctx, e.field(Field::Left)?)?;
            let r = flag_value(ctx, e.field(Field::Right)?)?;
            Some(l | r)
        }
        "field_access" => {
            let obj = e.field(Field::Object)?;
            let f = e.field(Field::Field)?;
            if !util::names_type(ctx, obj, "java.util.regex.Pattern") {
                return None;
            }
            flag_bit(ctx.text(f))
        }
        "identifier" => {
            let name = ctx.text(e);
            let bit = flag_bit(name)?;
            let imported = ctx.imports().iter().any(|i| {
                i.is_static
                    && ((i.is_wildcard && i.path == "java.util.regex.Pattern")
                        || (!i.is_wildcard
                            && i.path.strip_prefix("java.util.regex.Pattern.") == Some(name)))
            });
            imported.then_some(bit)
        }
        "decimal_integer_literal" | "hex_integer_literal" => {
            let v = syntax::int_literal_value(e, ctx.source())?;
            u32::try_from(v).ok().filter(|v| *v <= 0x1ff)
        }
        _ => None,
    }
}

fn flag_bit(name: &str) -> Option<u32> {
    Some(match name {
        "UNIX_LINES" => UNIX_LINES,
        "CASE_INSENSITIVE" => CASE_INSENSITIVE,
        "COMMENTS" => COMMENTS,
        "MULTILINE" => MULTILINE,
        "LITERAL" => LITERAL,
        "DOTALL" => DOTALL,
        "UNICODE_CASE" => UNICODE_CASE,
        "CANON_EQ" => CANON_EQ,
        "UNICODE_CHARACTER_CLASS" => UNICODE_CHARACTER_CLASS,
        _ => return None,
    })
}

const VALIDATION_PATTERNS: &[&str] = &[
    "javax.validation.constraints.Pattern",
    "jakarta.validation.constraints.Pattern",
];

/// `@Pattern(regexp = "…")` of Bean Validation (the value must match as a whole). Annotations
/// with `flags` are skipped.
fn from_annotation<'t>(
    ctx: &Ctx<'t>,
    ann: Node<'t>,
    wanted: &impl Fn(&str) -> bool,
) -> Option<RegexUse<'t>> {
    let name = ctx.text(ann.field(Field::Name)?);
    if !name.ends_with("Pattern") {
        return None;
    }
    let known = if name == "Pattern" {
        ctx.imports().iter().any(|i| {
            !i.is_static
                && if i.is_wildcard {
                    VALIDATION_PATTERNS
                        .iter()
                        .any(|p| p.strip_suffix(".Pattern") == Some(i.path))
                } else {
                    VALIDATION_PATTERNS.contains(&i.path)
                }
        }) && !ctx
            .imports()
            .iter()
            .any(|i| !i.is_static && !i.is_wildcard && i.path == "java.util.regex.Pattern")
    } else {
        VALIDATION_PATTERNS.contains(&name)
    };
    if !known {
        return None;
    }
    let args = ann.field(Field::Arguments)?;
    let mut regexp = None;
    for pair in named_children(args) {
        if kind(pair) != "element_value_pair" {
            continue;
        }
        let key = ctx.text(pair.field(Field::Key)?);
        match key {
            "regexp" => regexp = pair.field(Field::Value),
            "flags" => return None,
            _ => {}
        }
    }
    let value = regexp?;
    let pattern = util::string_constant(ctx, value)?;
    if !wanted(&pattern) {
        return None;
    }
    Some(RegexUse {
        node: value,
        pattern,
        flags: 0,
        usage: Usage::Full,
        split: false,
        empty_replacement: false,
    })
}
