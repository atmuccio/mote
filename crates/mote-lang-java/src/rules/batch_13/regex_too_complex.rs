//! `java/regex-too-complex` — a literal regular expression whose structure is too complex to
//! read and maintain.

use super::regex_syntax::{self as rx, Rx, K};
use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["regex", "complexity"],
    params: &[ParamSpec {
        name: "max",
        default: ParamDefault::Int(20),
        description: "Highest complexity score accepted for one regular expression.",
    }],
    remediation_minutes: 20,
    description: r#"Detects a constant regular expression — passed to `Pattern.compile`,
`Pattern.matches`, `String.matches`, `replaceAll`, `replaceFirst`, `split` or Bean Validation's
`@Pattern(regexp = …)` — whose structure scores above a threshold (`max`, 20 by default).

The score uses the same scale as `js/regex-too-complex`. Each of the following adds one point
plus one per level of group nesting around it: an alternative after the first (a list of plain
words such as `(GET|PUT|POST)` counts as one alternative), a variable repetition of a group
(`(…)*`, `(…)?`) and a lookaround. A variable repetition of a single character, class or
escape (`\s+`, `[0-9]*`), a character class and a back-reference add one point each. Fixed
repetitions (`\d{4}`), literal characters, escapes and anchors are free, so a long but flat
pattern scores low while nested alternations of repeated groups score high.

**Why it matters.** A regular expression has no names, comments or structure to guide the
reader: every nested group and alternative multiplies the cases to keep in mind. Complex
patterns are where subtle mistakes hide (an alternative that never matches, a quantifier that
backtracks for seconds on long input), and they are the part of the code people least dare to
change.

**How to fix.** Split the pattern into several simpler checks, build it from named constants
(`String DATE = "\\d{4}-\\d{2}-\\d{2}";`), use comments mode (`Pattern.COMMENTS`) to lay it out
with explanations, or replace it with a small parser.

```java
// Noncompliant
Pattern.compile("^(?:(?:25[0-5]|2[0-4]\\d|1\\d\\d|[1-9]?\\d)\\.){3}(?:25[0-5]|2[0-4]\\d|1\\d\\d|[1-9]?\\d)$");

// Compliant
private static final String OCTET = "(?:25[0-5]|2[0-4]\\d|1\\d\\d|[1-9]?\\d)";
Pattern.compile("^" + OCTET + "(?:\\." + OCTET + "){3}$");
```

The compliant form builds the same pattern, and is still reported when its total score is
above the threshold: split it into separate checks, or raise `max`, when the parts are named.
Not reported: patterns built at run time, patterns compiled with `Pattern.COMMENTS` (they are
already laid out for reading) or `LITERAL`, and patterns the parser does not model."#,
    ..RuleMeta::new(
        "java/regex-too-complex",
        "java",
        "Regular expression too complex",
    )
};

pub struct RegexTooComplex;

const CALL_NAMES: &[&str] = &["compile", "matches", "replaceAll", "replaceFirst", "split"];

const VALIDATION_PATTERNS: &[&str] = &[
    "javax.validation.constraints.Pattern",
    "jakarta.validation.constraints.Pattern",
];

impl<'t> JavaRule<'t> for RegexTooComplex {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation", "annotation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        CALL_NAMES
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let found = if kind(node) == "annotation" {
            from_annotation(ctx, node)
        } else {
            from_call(ctx, node)
        };
        let Some((at, pattern)) = found else {
            return;
        };
        let max = ctx.param_i64("max").max(0) as u64;
        let Some(re) = rx::parse(&pattern, 0) else {
            return;
        };
        let score = complexity(&re.root, 0);
        if score > max {
            ctx.emit(
                at,
                format!("Simplify this regular expression (complexity {score}, above {max}): split it into simpler checks or named parts"),
            )
            .value(score as f64);
        }
    }
}

/// The complexity score of a pattern node at a group nesting depth. The scale is the one of
/// `js/regex-too-complex`, so that both languages flag the same patterns.
fn complexity(r: &Rx, nesting: u64) -> u64 {
    match &r.k {
        K::Empty | K::Char(..) | K::Assert(..) | K::Opaque { .. } => 0,
        K::Set(_, _, origin) => match origin {
            rx::SetKind::Class(_) => 1,
            _ => 0,
        },
        K::BackRef(_) => 1,
        K::Seq(items) => items.iter().map(|i| complexity(i, nesting)).sum(),
        K::Alt(branches) => {
            // A list of plain words (`(GET|PUT|POST)`) reads as one choice: literal branches
            // count once together, branches with structure one each.
            let plain = branches.iter().filter(|b| is_plain(b)).count() as u64;
            let structured = branches.len() as u64 - plain;
            let choices = structured + u64::from(plain > 0);
            choices.saturating_sub(1) * (1 + nesting)
                + branches.iter().map(|b| complexity(b, nesting)).sum::<u64>()
        }
        K::Group(g, body) => {
            if g.is_lookaround() {
                1 + nesting + complexity(body, nesting + 1)
            } else {
                complexity(body, nesting + 1)
            }
        }
        K::Repeat { body, min, max, .. } => {
            // Fixed counts (`\d{4}`) are free; repeating a group costs more the deeper it sits,
            // repeating a single character, class or escape reads the same at any depth.
            let own = if *max == Some(*min) {
                0
            } else if matches!(body.k, K::Group(..)) {
                1 + nesting
            } else {
                1
            };
            own + complexity(body, nesting)
        }
    }
}

/// A branch made only of literal characters (or nothing).
fn is_plain(r: &Rx) -> bool {
    match &r.k {
        K::Empty | K::Char(..) => true,
        K::Seq(items) => items.iter().all(|i| matches!(i.k, K::Char(..))),
        _ => false,
    }
}

/// The pattern argument of a regex-taking JDK call, with its constant value.
fn from_call<'t>(ctx: &Ctx<'t>, call: Node<'t>) -> Option<(Node<'t>, String)> {
    let name = ctx.call_name(call)?;
    let n = util::arg_count(call);
    let is_static = match (name, n) {
        ("compile", 1 | 2) | ("matches", 2) => true,
        ("matches", 1) | ("replaceAll" | "replaceFirst", 2) | ("split", 1 | 2) => false,
        _ => return None,
    };
    let arg0 = util::arg(call, 0)?;
    if !matches!(
        kind(syntax::strip_parens(arg0)),
        "string_literal" | "binary_expression" | "identifier"
    ) {
        return None;
    }
    if name == "compile" && n == 2 {
        let flags = ctx.text(util::arg(call, 1)?);
        if flags.contains("COMMENTS") || flags.contains("LITERAL") || flags.contains("CANON_EQ") {
            return None;
        }
        // A computed flags value could hold them.
        if !flags.split(['|', '+']).all(|f| {
            util::last_segment(f)
                .chars()
                .all(|c| c.is_ascii_uppercase() || c == '_')
        }) {
            return None;
        }
    }
    let recv = call.field(Field::Object)?;
    let pattern = util::string_constant(ctx, arg0, 0)?;
    if is_static {
        if !util::names_type(ctx, recv, "java.util.regex.Pattern") {
            return None;
        }
    } else if !ctx.type_of(recv).is_some_and(|t| t.is_string()) {
        return None;
    }
    Some((arg0, pattern))
}

/// `@Pattern(regexp = "…")` of Bean Validation; annotations with `flags` are skipped.
fn from_annotation<'t>(ctx: &Ctx<'t>, ann: Node<'t>) -> Option<(Node<'t>, String)> {
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
        })
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
        match ctx.text(pair.field(Field::Key)?) {
            "regexp" => regexp = pair.field(Field::Value),
            "flags" => return None,
            _ => {}
        }
    }
    let value = regexp?;
    let pattern = util::string_constant(ctx, value, 0)?;
    Some((value, pattern))
}
