//! Engine-implemented rules.
//!
//! *Generic* rules run on the language-neutral data a plugin returns ([`FileAnalysis`] functions
//! and comments) plus the engine's lexer output, so every language gets them for free. Their ids
//! are instantiated per language: `java/cognitive-complexity`, `js/cognitive-complexity`, …
//!
//! *Meta* rules (language `mote`) report on the analysis itself: suppressions, parse errors,
//! internal errors. They are produced by the suppression and analysis modules.

use crate::lexer::LexOutput;
use mote_core::{
    Comment, FileAnalysis, FunctionMetric, LexerSpec, ParamDefault, ParamSpec, Quality, RawFinding,
    RuleKind, RuleMeta, RuleSet, RuleStatus, Severity, TextRange,
};
use rustc_hash::FxHashMap;
use std::borrow::Cow;
use std::sync::{Mutex, OnceLock};

/// The generic rules, identified by the part of their id after the language prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GenericRule {
    CognitiveComplexity,
    CyclomaticComplexity,
    FunctionTooLong,
    TooManyParameters,
    DeepNesting,
    FileTooLong,
    TodoComment,
    FixmeComment,
    CommentedOutCode,
    DuplicatedBlock,
}

impl GenericRule {
    /// All generic rules in catalog order.
    pub const ALL: [GenericRule; 10] = [
        GenericRule::CognitiveComplexity,
        GenericRule::CyclomaticComplexity,
        GenericRule::FunctionTooLong,
        GenericRule::TooManyParameters,
        GenericRule::DeepNesting,
        GenericRule::FileTooLong,
        GenericRule::TodoComment,
        GenericRule::FixmeComment,
        GenericRule::CommentedOutCode,
        GenericRule::DuplicatedBlock,
    ];

    /// The id suffix, e.g. `cognitive-complexity`.
    pub fn suffix(self) -> &'static str {
        match self {
            GenericRule::CognitiveComplexity => "cognitive-complexity",
            GenericRule::CyclomaticComplexity => "cyclomatic-complexity",
            GenericRule::FunctionTooLong => "function-too-long",
            GenericRule::TooManyParameters => "too-many-parameters",
            GenericRule::DeepNesting => "deep-nesting",
            GenericRule::FileTooLong => "file-too-long",
            GenericRule::TodoComment => "todo-comment",
            GenericRule::FixmeComment => "fixme-comment",
            GenericRule::CommentedOutCode => "commented-out-code",
            GenericRule::DuplicatedBlock => "duplicated-block",
        }
    }

    fn template(self) -> RuleMeta {
        let base = RuleMeta::new("", "", "");
        match self {
            GenericRule::CognitiveComplexity => RuleMeta {
                name: "Cognitive complexity of functions should not be too high",
                severity: Severity::Medium,
                tags: &["brain-overload", "complexity"],
                params: &[ParamSpec {
                    name: "threshold",
                    default: ParamDefault::Int(15),
                    description: "Highest cognitive complexity allowed for one function",
                }],
                description: DESC_COGNITIVE,
                remediation_minutes: 10,
                ..base
            },
            GenericRule::CyclomaticComplexity => RuleMeta {
                name: "Cyclomatic complexity of functions should not be too high",
                severity: Severity::Low,
                default_enabled: false,
                tags: &["complexity"],
                params: &[ParamSpec {
                    name: "threshold",
                    default: ParamDefault::Int(10),
                    description: "Highest cyclomatic complexity allowed for one function",
                }],
                description: DESC_CYCLOMATIC,
                remediation_minutes: 10,
                ..base
            },
            GenericRule::FunctionTooLong => RuleMeta {
                name: "Functions should not have too many lines of code",
                severity: Severity::Low,
                tags: &["brain-overload", "size"],
                params: &[ParamSpec {
                    name: "max_lines",
                    default: ParamDefault::Int(80),
                    description: "Most lines of code allowed in one function",
                }],
                description: DESC_FUNCTION_TOO_LONG,
                remediation_minutes: 20,
                ..base
            },
            GenericRule::TooManyParameters => RuleMeta {
                name: "Functions should not have too many parameters",
                severity: Severity::Medium,
                tags: &["brain-overload"],
                params: &[ParamSpec {
                    name: "max",
                    default: ParamDefault::Int(7),
                    description: "Most parameters allowed for one function",
                }],
                description: DESC_TOO_MANY_PARAMS,
                remediation_minutes: 20,
                ..base
            },
            GenericRule::DeepNesting => RuleMeta {
                name: "Control flow should not be nested too deeply",
                severity: Severity::Medium,
                tags: &["brain-overload", "complexity"],
                params: &[ParamSpec {
                    name: "max",
                    default: ParamDefault::Int(4),
                    description:
                        "Deepest nesting of control-flow statements allowed in one function",
                }],
                description: DESC_DEEP_NESTING,
                remediation_minutes: 10,
                ..base
            },
            GenericRule::FileTooLong => RuleMeta {
                name: "Files should not have too many lines of code",
                severity: Severity::Low,
                tags: &["brain-overload", "size"],
                params: &[ParamSpec {
                    name: "max_lines",
                    default: ParamDefault::Int(1000),
                    description: "Most lines of code allowed in one file",
                }],
                description: DESC_FILE_TOO_LONG,
                remediation_minutes: 60,
                ..base
            },
            GenericRule::TodoComment => RuleMeta {
                name: "TODO comments should be resolved",
                severity: Severity::Low,
                tags: &["todo"],
                description: DESC_TODO,
                remediation_minutes: 10,
                ..base
            },
            GenericRule::FixmeComment => RuleMeta {
                name: "FIXME, XXX and HACK comments should be resolved",
                severity: Severity::Medium,
                tags: &["todo"],
                description: DESC_FIXME,
                remediation_minutes: 20,
                ..base
            },
            GenericRule::CommentedOutCode => RuleMeta {
                name: "Commented-out code should be removed",
                severity: Severity::Low,
                tags: &["unused", "clutter"],
                description: DESC_COMMENTED_OUT,
                remediation_minutes: 5,
                ..base
            },
            GenericRule::DuplicatedBlock => RuleMeta {
                name: "Blocks of code should not be duplicated",
                severity: Severity::Low,
                tags: &["duplication"],
                description: DESC_DUPLICATED,
                remediation_minutes: 15,
                ..base
            },
        }
    }
}

/// Engine meta rules (language `mote`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MetaRule {
    UnusedSuppression,
    BlanketSuppression,
    SuppressionWithoutReason,
    ParseError,
    InternalError,
}

impl MetaRule {
    /// All meta rules in catalog order.
    pub const ALL: [MetaRule; 5] = [
        MetaRule::UnusedSuppression,
        MetaRule::BlanketSuppression,
        MetaRule::SuppressionWithoutReason,
        MetaRule::ParseError,
        MetaRule::InternalError,
    ];

    /// Full rule id, e.g. `mote/unused-suppression`.
    pub fn id(self) -> &'static str {
        self.meta().id
    }

    /// Static metadata.
    pub fn meta(self) -> &'static RuleMeta {
        match self {
            MetaRule::UnusedSuppression => &META_UNUSED,
            MetaRule::BlanketSuppression => &META_BLANKET,
            MetaRule::SuppressionWithoutReason => &META_NO_REASON,
            MetaRule::ParseError => &META_PARSE_ERROR,
            MetaRule::InternalError => &META_INTERNAL_ERROR,
        }
    }

    /// Look a meta rule up by id.
    pub fn from_id(id: &str) -> Option<MetaRule> {
        MetaRule::ALL.into_iter().find(|m| m.id() == id)
    }
}

const fn meta_rule(id: &'static str, name: &'static str, description: &'static str) -> RuleMeta {
    RuleMeta {
        id,
        name,
        language: "mote",
        kind: RuleKind::CodeSmell,
        quality: Quality::Maintainability,
        severity: Severity::Low,
        status: RuleStatus::Stable,
        default_enabled: true,
        tags: &["mote"],
        params: &[],
        description,
        remediation_minutes: 2,
        fixable: false,
    }
}

static META_UNUSED: RuleMeta = meta_rule(
    "mote/unused-suppression",
    "Suppressions should suppress something",
    DESC_UNUSED_SUPPRESSION,
);
static META_BLANKET: RuleMeta = meta_rule(
    "mote/blanket-suppression",
    "Suppressions should name the rules they silence",
    DESC_BLANKET,
);
static META_NO_REASON: RuleMeta = meta_rule(
    "mote/suppression-without-reason",
    "Suppressions should give a reason",
    DESC_NO_REASON,
);
static META_PARSE_ERROR: RuleMeta = meta_rule(
    "mote/parse-error",
    "Files should parse without syntax errors",
    DESC_PARSE_ERROR,
);
static META_INTERNAL_ERROR: RuleMeta = meta_rule(
    "mote/internal-error",
    "Files should be analyzable",
    DESC_INTERNAL_ERROR,
);

/// Interned generic rule ids and metadata for one language.
#[derive(Debug)]
pub struct GenericSet {
    pub language: &'static str,
    /// `(rule, meta)` in catalog order.
    pub rules: Vec<(GenericRule, &'static RuleMeta)>,
}

impl GenericSet {
    /// Full id of `rule` for this language.
    pub fn id(&self, rule: GenericRule) -> &'static str {
        self.rules
            .iter()
            .find(|(r, _)| *r == rule)
            .map(|(_, m)| m.id)
            .unwrap_or("")
    }
}

/// Generic rule metadata for `language`, created once per process (ids are leaked to get the
/// `&'static str` the core model requires; the set is interned so this happens once per language).
pub fn generic_set(language: &'static str) -> &'static GenericSet {
    static SETS: OnceLock<Mutex<FxHashMap<&'static str, &'static GenericSet>>> = OnceLock::new();
    let sets = SETS.get_or_init(|| Mutex::new(FxHashMap::default()));
    let mut guard = match sets.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(set) = guard.get(language) {
        return set;
    }
    let rules = GenericRule::ALL
        .iter()
        .map(|&rule| {
            let id: &'static str =
                Box::leak(format!("{language}/{}", rule.suffix()).into_boxed_str());
            let meta: &'static RuleMeta = Box::leak(Box::new(RuleMeta {
                id,
                language,
                ..rule.template()
            }));
            (rule, meta)
        })
        .collect();
    let set: &'static GenericSet = Box::leak(Box::new(GenericSet { language, rules }));
    guard.insert(language, set);
    set
}

/// Inputs for evaluating generic rules on one file.
pub struct GenericInput<'a> {
    pub set: &'a GenericSet,
    /// Generic rules the plugin implements itself (skipped here).
    pub overridden: &'a [GenericRule],
    pub rules: &'a RuleSet,
    pub analysis: &'a FileAnalysis,
    pub comments: &'a [Comment],
    pub lex: &'a LexOutput,
    pub spec: &'a LexerSpec,
}

/// Evaluate every enabled generic rule except `duplicated-block` (which runs in the reduce phase).
pub fn evaluate(input: &GenericInput<'_>) -> Vec<RawFinding> {
    let mut out = Vec::new();
    let enabled = |rule: GenericRule| -> Option<&'static str> {
        if input.overridden.contains(&rule) {
            return None;
        }
        let id = input.set.id(rule);
        input.rules.is_enabled(id).then_some(id)
    };

    if let Some(id) = enabled(GenericRule::CognitiveComplexity) {
        let threshold = input.rules.param_i64(id, "threshold", 15);
        for f in &input.analysis.functions {
            if i64::from(f.cognitive) > threshold {
                out.push(fn_finding(
                    id,
                    f,
                    format!(
                        "Refactor `{}` to reduce its cognitive complexity from {} to {} or less",
                        f.name, f.cognitive, threshold
                    ),
                    f.cognitive,
                ));
            }
        }
    }
    if let Some(id) = enabled(GenericRule::CyclomaticComplexity) {
        let threshold = input.rules.param_i64(id, "threshold", 10);
        for f in &input.analysis.functions {
            if i64::from(f.cyclomatic) > threshold {
                out.push(fn_finding(
                    id,
                    f,
                    format!(
                        "Refactor `{}` to reduce its cyclomatic complexity from {} to {} or less",
                        f.name, f.cyclomatic, threshold
                    ),
                    f.cyclomatic,
                ));
            }
        }
    }
    if let Some(id) = enabled(GenericRule::FunctionTooLong) {
        let max = input.rules.param_i64(id, "max_lines", 80);
        for f in &input.analysis.functions {
            let loc = code_lines_between(input.lex, f.start_line, f.end_line);
            if i64::from(loc) > max {
                out.push(fn_finding(
                    id,
                    f,
                    format!(
                        "Split `{}` into smaller functions; it has {} lines of code, more than {}",
                        f.name, loc, max
                    ),
                    loc,
                ));
            }
        }
    }
    if let Some(id) = enabled(GenericRule::TooManyParameters) {
        let max = input.rules.param_i64(id, "max", 7);
        for f in &input.analysis.functions {
            if i64::from(f.params) > max {
                out.push(fn_finding(
                    id,
                    f,
                    format!(
                        "Reduce the number of parameters of `{}` from {} to {} or fewer",
                        f.name, f.params, max
                    ),
                    f.params,
                ));
            }
        }
    }
    if let Some(id) = enabled(GenericRule::DeepNesting) {
        let max = input.rules.param_i64(id, "max", 4);
        for f in &input.analysis.functions {
            if i64::from(f.max_nesting) > max {
                out.push(fn_finding(
                    id,
                    f,
                    format!(
                        "Reduce the nesting depth in `{}` from {} to {} or less",
                        f.name, f.max_nesting, max
                    ),
                    f.max_nesting,
                ));
            }
        }
    }
    if let Some(id) = enabled(GenericRule::FileTooLong) {
        let max = input.rules.param_i64(id, "max_lines", 1000);
        let loc = input.lex.ncloc();
        if i64::from(loc) > max {
            out.push(RawFinding {
                rule_id: id.to_string(),
                message: format!("Split this file into smaller units; it has {loc} lines of code, more than {max}"),
                range: TextRange::line(1),
                symbol: None,
                value: Some(f64::from(loc)),
                secondary: Vec::new(),
                fix: None,
            });
        }
    }
    let todo = enabled(GenericRule::TodoComment);
    let fixme = enabled(GenericRule::FixmeComment);
    if todo.is_some() || fixme.is_some() {
        task_markers(input, todo, fixme, &mut out);
    }
    if let Some(id) = enabled(GenericRule::CommentedOutCode) {
        commented_out_code(input, id, &mut out);
    }
    out
}

fn fn_finding(id: &str, f: &FunctionMetric, message: String, value: u32) -> RawFinding {
    let range = if f.range.start_line == 0 {
        TextRange::line(f.start_line.max(1))
    } else {
        f.range
    };
    RawFinding {
        rule_id: id.to_string(),
        message,
        range,
        symbol: Some(f.name.clone()),
        value: Some(f64::from(value)),
        secondary: Vec::new(),
        fix: None,
    }
}

fn code_lines_between(lex: &LexOutput, start: u32, end: u32) -> u32 {
    if end < start {
        return 0;
    }
    let lo = lex.code_lines.partition_point(|&l| l < start);
    let hi = lex.code_lines.partition_point(|&l| l <= end);
    (hi - lo) as u32
}

/// Innermost function whose span contains `line`.
pub fn enclosing_function(functions: &[FunctionMetric], line: u32) -> Option<&FunctionMetric> {
    functions
        .iter()
        .filter(|f| f.start_line <= line && line <= f.end_line)
        .max_by_key(|f| (f.start_line, std::cmp::Reverse(f.end_line)))
}

fn is_word_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// Byte offsets of `marker` in `line` at word boundaries.
fn find_marker(line: &str, marker: &str) -> Option<usize> {
    let b = line.as_bytes();
    let mut from = 0;
    while let Some(pos) = line[from..].find(marker) {
        let start = from + pos;
        let end = start + marker.len();
        let before_ok = start == 0 || !is_word_byte(b[start - 1]);
        let after_ok = end >= b.len() || !is_word_byte(b[end]);
        if before_ok && after_ok {
            return Some(start);
        }
        from = end;
    }
    None
}

/// Byte offset of the first occurrence of `marker` in `line` that is used as a marker rather
/// than as a placeholder (see [`is_placeholder`]).
fn find_task_marker(line: &str, marker: &str) -> Option<usize> {
    let b = line.as_bytes();
    let mut from = 0;
    while let Some(pos) = line.get(from..).and_then(|rest| rest.find(marker)) {
        let start = from + pos;
        let end = start + marker.len();
        // Word boundaries are checked against the whole line, not the searched tail.
        let bounded = (start == 0 || !is_word_byte(b[start - 1]))
            && (end >= b.len() || !is_word_byte(b[end]));
        if bounded && !is_placeholder(line, start, end) {
            return Some(start);
        }
        from = end;
    }
    None
}

/// Whether the marker word at `start..end` stands for something else, as placeholder text does:
/// quoted (`file name "XXX"`), a member name (`Thread.XXX`, `XXX.value`), inside inline
/// code or emphasis (`set<em>XXX</em>()`, `{@code XXX}`), or one of a run of X placeholders
/// (`XXXX XXX XX n`, a record format).
fn is_placeholder(line: &str, start: usize, end: usize) -> bool {
    let (before, after) = (&line[..start], &line[end..]);
    let b = before.as_bytes();
    if before.ends_with(['"', '\'', '`']) {
        return true;
    }
    if before.ends_with('.') && b.len() >= 2 && is_word_byte(b[b.len() - 2]) {
        return true;
    }
    if after.starts_with('.') && after.as_bytes().get(1).is_some_and(u8::is_ascii_alphabetic) {
        return true;
    }
    if before.ends_with("{@code ") || before.ends_with("{@literal ") {
        return true;
    }
    for tag in ["em", "i", "code", "var", "tt", "samp"] {
        if before.ends_with(&format!("<{tag}>")) && after.starts_with(&format!("</{tag}>")) {
            return true;
        }
    }
    let x_run = |w: &str| w.len() >= 2 && w.bytes().all(|c| c == b'X');
    let prev = before
        .trim_end()
        .rsplit(|c: char| !c.is_ascii_alphanumeric())
        .next()
        .unwrap_or("");
    let next = after
        .trim_start()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .next()
        .unwrap_or("");
    x_run(prev) || x_run(next)
}

/// The markers of `fixme-comment`.
const FIXME_MARKERS: [&str; 3] = ["FIXME", "XXX", "HACK"];

fn task_markers(
    input: &GenericInput<'_>,
    todo: Option<&'static str>,
    fixme: Option<&'static str>,
    out: &mut Vec<RawFinding>,
) {
    for c in input.comments {
        // A marker on a line is in the comment's text: most comments have none and are not
        // split into lines at all.
        let todo = todo.filter(|_| c.text.contains("TODO"));
        let fixme = fixme.filter(|_| FIXME_MARKERS.iter().any(|m| c.text.contains(m)));
        if todo.is_none() && fixme.is_none() {
            continue;
        }
        for (i, line) in c.text.split('\n').enumerate() {
            let line_no = c.start_line + i as u32;
            let col_base = if i == 0 { c.start_col } else { 1 };
            let symbol =
                || enclosing_function(&input.analysis.functions, line_no).map(|f| f.name.clone());
            if let Some(id) = todo {
                if let Some(pos) = find_marker(line, "TODO") {
                    let col = col_base + line[..pos].chars().count() as u32;
                    out.push(RawFinding {
                        rule_id: id.to_string(),
                        message: "Complete the task noted in this TODO comment".to_string(),
                        range: TextRange {
                            start_line: line_no,
                            start_col: col,
                            end_line: line_no,
                            end_col: col + 4,
                        },
                        symbol: symbol(),
                        value: None,
                        secondary: Vec::new(),
                        fix: None,
                    });
                }
            }
            if let Some(id) = fixme {
                let hit = FIXME_MARKERS
                    .iter()
                    .filter_map(|m| find_task_marker(line, m).map(|p| (p, *m)))
                    .min_by_key(|(p, _)| *p);
                if let Some((pos, marker)) = hit {
                    let col = col_base + line[..pos].chars().count() as u32;
                    out.push(RawFinding {
                        rule_id: id.to_string(),
                        message: format!("Resolve the problem flagged by this {marker} comment"),
                        range: TextRange {
                            start_line: line_no,
                            start_col: col,
                            end_line: line_no,
                            end_col: col + marker.len() as u32,
                        },
                        symbol: symbol(),
                        value: None,
                        secondary: Vec::new(),
                        fix: None,
                    });
                }
            }
        }
    }
}

const CODE_KEYWORDS: &[&str] = &[
    "if",
    "for",
    "while",
    "return",
    "import",
    "var",
    "let",
    "const",
    "public",
    "private",
    "protected",
    "else",
    "switch",
    "case",
    "try",
    "catch",
    "finally",
    "throw",
    "new",
    "static",
    "final",
    "class",
    "function",
    "package",
    "do",
    "break",
    "continue",
    "export",
];

/// Heuristic: does this (comment-stripped) line look like source code?
/// Remove Javadoc/JSDoc inline tags (`{@link Foo#bar()}`), which end in `}` but are prose.
fn strip_inline_doc_tags(line: &str) -> Cow<'_, str> {
    if !line.contains("{@") {
        return Cow::Borrowed(line);
    }
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(pos) = rest.find("{@") {
        out.push_str(&rest[..pos]);
        match rest[pos..].find('}') {
            Some(end) => {
                out.push_str("tag");
                rest = &rest[pos + end + 1..];
            }
            None => {
                rest = "";
            }
        }
    }
    out.push_str(rest);
    Cow::Owned(out)
}

fn ends_like_code(t: &str) -> bool {
    (t.ends_with(';') && !ends_with_html_entity(t))
        || t.ends_with('{')
        || (t.ends_with('}') && (t.contains('{') || t.starts_with('}')))
}

/// Prose about escaping ends with an HTML entity (`escape " to &quot;`, `&#39;`), whose semicolon
/// is not a statement end.
fn ends_with_html_entity(t: &str) -> bool {
    let Some(body) = t.strip_suffix(';') else {
        return false;
    };
    let Some(amp) = body.rfind('&') else {
        return false;
    };
    // `x = a&b;` is a tight bitwise operator: prose puts a space or a quote before an entity.
    if body[..amp]
        .chars()
        .next_back()
        .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$')
    {
        return false;
    }
    let name = &body[amp + 1..];
    let named = name.starts_with(|c: char| c.is_ascii_alphabetic())
        && name.chars().all(|c| c.is_ascii_alphanumeric());
    let decimal = name
        .strip_prefix('#')
        .is_some_and(|d| !d.is_empty() && d.chars().all(|c| c.is_ascii_digit()));
    let hex = name
        .strip_prefix("#x")
        .or_else(|| name.strip_prefix("#X"))
        .is_some_and(|h| !h.is_empty() && h.chars().all(|c| c.is_ascii_hexdigit()));
    named || decimal || hex
}

fn looks_like_code(line: &str) -> bool {
    let cleaned = strip_inline_doc_tags(line);
    let t = cleaned.trim();
    if t.is_empty() {
        return false;
    }
    if ends_like_code(t) {
        // "}" alone or "};" etc. are code; so is "foo();".
        return true;
    }
    let first = t
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .next()
        .unwrap_or("");
    if CODE_KEYWORDS.contains(&first) {
        // Require some code punctuation after the keyword; "return early if empty" is prose.
        let rest = &t[first.len()..];
        return rest.trim_start().starts_with('(')
            || rest.contains(['=', '(', '{', ';', '.']) && !rest.contains(". ");
    }
    false
}

fn strong_code_line(line: &str) -> bool {
    let cleaned = strip_inline_doc_tags(line);
    let t = cleaned.trim();
    ends_like_code(t) && t.contains(['(', '=', '.', '{', '}', ';'])
}

fn is_directive_or_header(text: &str) -> bool {
    const MARKERS: &[&str] = &[
        "NOSONAR",
        "Copyright",
        "copyright",
        "License",
        "LICENSE",
        "SPDX-",
        "eslint-",
        "prettier-",
        "@ts-",
        "istanbul",
        "c8 ",
        "jshint",
        "global ",
        "expect:",
        "expect-next-line:",
        "sourceMappingURL",
        "<reference",
    ];
    // The engine's own directives, as whole words (`remote-ignore` is not one).
    const OWN: &[&str] = &["mote-ignore", "mote-test:"];
    MARKERS.iter().any(|m| text.contains(m))
        || OWN.iter().any(|m| {
            text.match_indices(m)
                .any(|(at, _)| crate::suppress::starts_word(text, at))
        })
}

fn commented_out_code(input: &GenericInput<'_>, id: &'static str, out: &mut Vec<RawFinding>) {
    let lc = input.spec.line_comment;
    let (bo, bc) = input.spec.block_comment;
    // Group consecutive own-line line comments into runs; block comments are their own runs.
    let mut runs: Vec<(u32, u32, Vec<&str>)> = Vec::new();
    let mut current: Option<(u32, u32, Vec<&str>)> = None;
    for c in input.comments {
        let text = c.text.as_str();
        let is_line = !lc.is_empty() && text.starts_with(lc);
        if is_line && !c.trailing {
            let body = &text[lc.len()..];
            match current.as_mut() {
                Some((_, end, lines)) if *end + 1 == c.start_line => {
                    *end = c.start_line;
                    lines.push(body);
                }
                _ => {
                    if let Some(r) = current.take() {
                        runs.push(r);
                    }
                    current = Some((c.start_line, c.start_line, vec![body]));
                }
            }
            continue;
        }
        if let Some(r) = current.take() {
            runs.push(r);
        }
        if c.trailing || bo.is_empty() || !text.starts_with(bo) {
            continue;
        }
        // Doc comments (`/**`) and file headers are prose by convention.
        if text[bo.len()..].starts_with('*') || c.start_line == 1 {
            continue;
        }
        let inner = text[bo.len()..]
            .strip_suffix(bc)
            .unwrap_or(&text[bo.len()..]);
        let lines = inner
            .split('\n')
            .map(|l| l.trim_start().trim_start_matches('*'))
            .collect::<Vec<_>>();
        runs.push((c.start_line, c.end_line, lines));
    }
    if let Some(r) = current.take() {
        runs.push(r);
    }

    for (start, end, lines) in runs {
        let joined = lines.join("\n");
        if is_directive_or_header(&joined) {
            continue;
        }
        let non_empty: Vec<&str> = lines
            .iter()
            .copied()
            .filter(|l| !l.trim().is_empty())
            .collect();
        if non_empty.is_empty() {
            continue;
        }
        let code = non_empty.iter().filter(|l| looks_like_code(l)).count();
        let strong = non_empty.iter().filter(|l| strong_code_line(l)).count();
        let is_code = if non_empty.len() == 1 {
            strong == 1 && non_empty[0].trim().len() > 1
        } else {
            strong >= 1 && code * 10 >= non_empty.len() * 6
        };
        if !is_code {
            continue;
        }
        let symbol = enclosing_function(&input.analysis.functions, start).map(|f| f.name.clone());
        out.push(RawFinding {
            rule_id: id.to_string(),
            message: "Remove this commented-out code".to_string(),
            range: TextRange {
                start_line: start,
                start_col: 1,
                end_line: end,
                end_col: 1,
            },
            symbol,
            value: None,
            secondary: Vec::new(),
            fix: None,
        });
    }
}

// ---------------------------------------------------------------------------------------------
// Descriptions (Markdown, original wording).

const DESC_COGNITIVE: &str = "\
Flags functions whose **cognitive complexity** is above a threshold (default 15).

Cognitive complexity estimates how hard a function is to *read*: every break in linear flow \
(`if`, loops, `catch`, `switch`, jumps, chains of mixed boolean operators) adds a point, and \
nesting makes each of those cost more. Functions that score high take longer to understand, are \
harder to test exhaustively and attract bugs when changed.

**How to fix:** extract well-named helper functions, return early instead of nesting, and replace \
flag-driven branching with separate functions or polymorphism.

```text
// Noncompliant: nested branches inside loops inside conditions
process(items) {
  if (enabled) {
    for (item of items) {
      if (item.ok) { if (item.big) { ... } else { ... } }
    }
  }
}

// Compliant: guard clause plus a helper
process(items) {
  if (!enabled) return;
  for (item of items) handle(item);
}
```
";

const DESC_CYCLOMATIC: &str = "\
Flags functions whose **cyclomatic complexity** (the number of independent paths through the \
function) is above a threshold (default 10). Off by default: cognitive complexity usually reflects \
readability better.

Many independent paths mean many test cases are needed to cover the function and a higher chance \
that some combination is wrong.

**How to fix:** split the function by responsibility, use lookup tables instead of long \
`if`/`else` chains, and move validation into dedicated helpers.

```text
// Noncompliant: one function handling every case
price(order) { if (a) ... else if (b) ... else if (c) ... /* 12 more */ }

// Compliant
price(order) { return RULES[order.kind](order); }
```
";

const DESC_FUNCTION_TOO_LONG: &str = "\
Flags functions with more than `max_lines` lines of code (default 80; blank and comment-only lines \
are not counted).

Long functions usually do several things at once. They are harder to name, to understand and to \
reuse, and changes to them are riskier.

**How to fix:** extract cohesive steps into separate, well-named functions.

```text
// Noncompliant
importFile(path) { /* parse, validate, transform, save, notify — 150 lines */ }

// Compliant
importFile(path) { save(transform(validate(parse(path)))); notify(path); }
```
";

const DESC_TOO_MANY_PARAMS: &str = "\
Flags functions that declare more than `max` parameters (default 7).

Long parameter lists are hard to call correctly (arguments of the same type are easily swapped) \
and often signal that the function does too much or that several values belong together.

**How to fix:** group related values into a parameter object, or split the function.

```text
// Noncompliant
createUser(first, last, email, phone, street, city, zip, country) { ... }

// Compliant
createUser(name, contact, address) { ... }
```
";

const DESC_DEEP_NESTING: &str = "\
Flags functions whose control-flow statements (`if`, loops, `switch`, `try`) are nested more than \
`max` levels deep (default 4).

Deeply nested code forces readers to keep many conditions in mind at once and hides the main path \
of the function.

**How to fix:** use guard clauses and early returns, invert conditions, and extract inner blocks \
into helper functions.

```text
// Noncompliant
if (a) { for (...) { if (b) { while (c) { if (d) { ... } } } } }

// Compliant
if (!a) return;
for (...) processItem(...);
```
";

const DESC_FILE_TOO_LONG: &str = "\
Flags files with more than `max_lines` lines of code (default 1000; blank and comment-only lines \
are not counted).

Very large files tend to mix responsibilities, are slow to navigate and review, and cause frequent \
merge conflicts.

**How to fix:** split the file along its responsibilities into smaller modules or classes.
";

const DESC_TODO: &str = "\
Flags comments containing a `TODO` marker.

A TODO records work that was knowingly left unfinished. Left in place, it silently becomes \
permanent and the missing behaviour may surprise users.

**How to fix:** do the work, or move it to the issue tracker and delete the comment.

```text
// Noncompliant
// TODO handle the empty list
```
";

const DESC_FIXME: &str = "\
Flags comments containing `FIXME`, `XXX` or `HACK` markers.

A marker word used as placeholder text is not reported: quoted (`file name \"XXX\"`), as a \
member name (`Thread.XXX`), inside inline code or emphasis (`set<em>XXX</em>()`), or in a run \
of X placeholders (`XXXX XXX XX n`).

These markers admit that the code is known to be wrong or fragile. Unlike a TODO, the current \
behaviour itself is suspect.

**How to fix:** correct the code and delete the marker, or track the problem in the issue tracker \
with enough context to fix it later.

```text
// Noncompliant
total = total * 1.1; // FIXME rounding is wrong for negative amounts
```
";

const DESC_COMMENTED_OUT: &str = "\
Flags runs of comments that contain source code rather than prose.

Commented-out code distracts readers, rots as the surrounding code changes and is never compiled or \
tested. Version control already keeps old code.

**How to fix:** delete it. If it documents an alternative, describe the alternative in prose.

```text
// Noncompliant
// int retries = 3;
// connect(host, retries);
connect(host);
```

Documentation comments, license headers and tool directives are ignored.
";

const DESC_DUPLICATED: &str = "\
Flags blocks of code that appear, token for token, in more than one place (by default at least 100 \
tokens spanning at least 10 lines; literal values and whitespace are ignored).

Duplicated code must be changed in several places at once; sooner or later one copy is missed and \
the copies drift apart.

**How to fix:** extract the shared logic into a function, class or module and call it from each \
place.
";

const DESC_UNUSED_SUPPRESSION: &str = "\
Flags `mote-ignore` comments that did not suppress any finding.

A suppression that no longer matches anything is noise, and it can hide a *future* finding that \
nobody reviewed.

**How to fix:** delete the suppression comment.

```text
// Noncompliant: nothing is reported on the next line any more
// mote-ignore[java/empty-catch-block]: legacy
doWork();
```
";

const DESC_BLANKET: &str = "\
Flags suppressions that do not name the rules they silence: `mote-ignore:` without a rule list, \
and `NOSONAR` comments when compatibility mode is on.

Blanket suppressions hide every current and future finding on the line, including real bugs.

**How to fix:** list the rule ids, e.g. `mote-ignore[java/magic-number]: port is fixed by spec`.
";

const DESC_NO_REASON: &str = "\
Flags `mote-ignore` suppressions without a reason after the colon (when \
`suppressions.require_reason` is on, the default).

A reason lets reviewers judge whether the suppression is still justified.

**How to fix:** add a short explanation: `// mote-ignore[js/no-console]: CLI output`.
";

const DESC_PARSE_ERROR: &str = "\
Reported once per file whose parser had to recover from syntax errors.

Results for such files can be incomplete: rules may miss code the parser could not understand. \
The cause is either invalid code or syntax the parser does not support yet.

**How to fix:** fix the syntax error, or report the unsupported syntax.
";

const DESC_INTERNAL_ERROR: &str = "\
Reported when a file could not be analyzed at all: it could not be read, or the analyzer failed on \
it. Never fails a quality gate.

**How to fix:** nothing to change in the code; please report the problem with the file attached if \
possible.
";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::{lex, LexOptions};
    use mote_core::ActiveRule;
    use std::collections::BTreeMap;

    fn spec() -> LexerSpec {
        LexerSpec {
            line_comment: "//",
            block_comment: ("/*", "*/"),
            text_blocks: false,
            template_literals: false,
            cpd_skip_line_prefixes: &[],
        }
    }

    fn rules(ids: &[&str]) -> RuleSet {
        let mut m = FxHashMap::default();
        for id in ids {
            m.insert(
                id.to_string(),
                ActiveRule {
                    severity: Severity::Low,
                    params: BTreeMap::new(),
                },
            );
        }
        RuleSet::new(m)
    }

    fn run(text: &str, rule: GenericRule, functions: Vec<FunctionMetric>) -> Vec<RawFinding> {
        let set = generic_set("tl");
        let lexed = lex(text, &spec(), LexOptions::default());
        let analysis = FileAnalysis {
            functions,
            ..Default::default()
        };
        let rs = rules(&[set.id(rule)]);
        evaluate(&GenericInput {
            set,
            overridden: &[],
            rules: &rs,
            analysis: &analysis,
            comments: &lexed.comments,
            lex: &lexed,
            spec: &spec(),
        })
    }

    #[test]
    fn generic_ids_are_interned() {
        let a = generic_set("tl");
        let b = generic_set("tl");
        assert!(std::ptr::eq(a, b));
        assert_eq!(
            a.id(GenericRule::CognitiveComplexity),
            "tl/cognitive-complexity"
        );
        assert!(a
            .rules
            .iter()
            .all(|(_, m)| !m.description.is_empty() && m.language == "tl"));
        assert!(
            !a.rules
                .iter()
                .find(|(r, _)| *r == GenericRule::CyclomaticComplexity)
                .unwrap()
                .1
                .default_enabled
        );
    }

    #[test]
    fn cognitive_complexity_threshold() {
        let f = |name: &str, cog| FunctionMetric {
            name: name.into(),
            range: TextRange::line(3),
            start_line: 3,
            end_line: 9,
            cognitive: cog,
            ..Default::default()
        };
        let out = run(
            "x",
            GenericRule::CognitiveComplexity,
            vec![f("a", 15), f("b", 23)],
        );
        assert_eq!(out.len(), 1);
        assert_eq!(
            out[0].message,
            "Refactor `b` to reduce its cognitive complexity from 23 to 15 or less"
        );
        assert_eq!(out[0].value, Some(23.0));
        assert_eq!(out[0].symbol.as_deref(), Some("b"));
    }

    #[test]
    fn fixme_placeholders_are_not_markers() {
        let text = "// the file name \"XXX\"
/** set<em>XXX</em>(boolean) and {@code XXX} */
// see Thread.XXX and XXX.value
// format (XXXX XXX XX n)
// \"XXX\" then XXX: real marker
// <b>XXX</b> tasks to do
// LKS - XXX revisit
// aXXXXXX glued
";
        let fixme = run(text, GenericRule::FixmeComment, vec![]);
        assert_eq!(
            fixme.iter().map(|f| f.range.start_line).collect::<Vec<_>>(),
            vec![5, 6, 7]
        );
        assert_eq!(fixme[0].range.start_col, 15);
    }

    #[test]
    fn todo_and_fixme_markers() {
        let text = "// TODO: a\n// TODOS are not markers\n/* x\n * FIXME later XXX */\n// hack HACK\nint TODO_x;\n";
        let todo = run(text, GenericRule::TodoComment, vec![]);
        assert_eq!(
            todo.iter().map(|f| f.range.start_line).collect::<Vec<_>>(),
            vec![1]
        );
        assert_eq!(todo[0].range.start_col, 4);
        let fixme = run(text, GenericRule::FixmeComment, vec![]);
        assert_eq!(
            fixme.iter().map(|f| f.range.start_line).collect::<Vec<_>>(),
            vec![4, 5]
        );
        assert!(fixme[0].message.contains("FIXME"));
        assert_eq!(fixme[0].range.start_col, 4);
    }

    #[test]
    fn commented_out_code_detection() {
        let text = "\
/* Copyright header int x; */
class A {
  // int retries = 3;
  // connect(host, retries);
  // if (x) {
  //   return;
  // }
  void a() {}
  // return early if the list is empty
  // Handles the case where
  // the input is null.
  /** @param x the value; */
  // foo();
  int y = 1; // z = 2;
  // mote-ignore[x]: foo();
  /*
   * This job calls {@link IRouterService#routeData()}
   */
  /* if (a) { b(); } */
  // see {@link Foo} and {@code bar}
  void b() {}
  // escape \" to &quot;

  // turn & into &amp;
  void c() {}
  // then ' becomes &#39;
  void d() {}
  // x = a & b;
  void e() {}
  // x = a&b;
}
";
        let out = run(text, GenericRule::CommentedOutCode, vec![]);
        let lines: Vec<(u32, u32)> = out
            .iter()
            .map(|f| (f.range.start_line, f.range.end_line))
            .collect();
        assert_eq!(lines, vec![(3, 7), (13, 13), (19, 19), (28, 28), (30, 30)]);
    }

    #[test]
    fn function_length_counts_code_lines() {
        let text = "f() {\n\n// c\na;\nb;\n}\n";
        let f = FunctionMetric {
            name: "f".into(),
            start_line: 1,
            end_line: 6,
            range: TextRange::line(1),
            ..Default::default()
        };
        let set = generic_set("tl");
        let lexed = lex(text, &spec(), LexOptions::default());
        let analysis = FileAnalysis {
            functions: vec![f],
            ..Default::default()
        };
        let id = set.id(GenericRule::FunctionTooLong);
        let mut m = FxHashMap::default();
        let mut params = BTreeMap::new();
        params.insert("max_lines".to_string(), mote_core::ParamValue::Int(3));
        m.insert(
            id.to_string(),
            ActiveRule {
                severity: Severity::Low,
                params,
            },
        );
        let rs = RuleSet::new(m);
        let out = evaluate(&GenericInput {
            set,
            overridden: &[],
            rules: &rs,
            analysis: &analysis,
            comments: &lexed.comments,
            lex: &lexed,
            spec: &spec(),
        });
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].value, Some(4.0));
    }
}
