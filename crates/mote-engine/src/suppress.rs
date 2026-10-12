//! Suppressions: comment directives, plugin-scoped suppressions, and the meta rules about them.
//!
//! Comment syntax (inside any comment):
//!
//! * `mote-ignore[rule-a, java/rule-b]: reason` — suppresses the listed rules on the comment's
//!   own line when the comment trails code, otherwise on the next line that contains code — and,
//!   when that line starts a run of annotations (`@Deprecated`, decorators), on the lines up to
//!   the declaration they annotate;
//! * `mote-ignore: reason` — the same, for every rule (a *blanket* suppression);
//! * `mote-ignore-file[rule]: reason` — for the whole file (a rule list is optional, but a
//!   file-wide blanket suppression is reported as blanket).
//!
//! Rule references may omit the language prefix (`empty-catch-block` means
//! `<language>/empty-catch-block`).
//!
//! Other tools' markers are honored per source with `suppressions.compat` ([`CompatSet`],
//! [`COMPAT_SOURCES`]): with `nosonar`, `NOSONAR` in a comment is a blanket suppression for its
//! line; a plugin suppression of kind `compat:<source>` applies when its source is selected (the
//! untagged legacy kind `compat` applies when any source is). Plugin suppressions of any other
//! kind (e.g. `annotation`) always apply. Compatibility suppressions are reported with the kind
//! `compat` (`nosonar` for `NOSONAR`) and are never checked for being unused.
//!
//! Suppressed findings stay in the results with [`SuppressionInfo`] set and never gate. Meta rule
//! findings (`mote/*`) cannot be suppressed.

use crate::fingerprint::anchor_text;
use crate::generic::MetaRule;
use crate::lexer::LexOutput;
use mote_core::{
    Comment, LineIndex, RawFinding, RuleSet, ScopedSuppression, SuppressionInfo, TextRange,
};
use serde::{Deserialize, Serialize};

/// The sources of other tools' suppressions that `suppressions.compat` can select, sorted:
///
/// * `checkstyle` — `// CHECKSTYLE:OFF` … `// CHECKSTYLE:ON` regions (Java);
/// * `eslint` — `eslint-disable…` directives (JavaScript/TypeScript);
/// * `intellij` — `//noinspection Name` comments and `@SuppressWarnings("Name")` with IntelliJ
///   inspection names (Java);
/// * `nosonar` — `NOSONAR` in a comment (every language);
/// * `pmd` — `// NOPMD`, `@SuppressWarnings("PMD")` and `@SuppressWarnings("PMD.Rule")` (Java);
/// * `spotbugs` — `@SuppressFBWarnings` (Java);
/// * `suppress-warnings-all` — `@SuppressWarnings("all")` (Java).
pub const COMPAT_SOURCES: [&str; 7] = [
    "checkstyle",
    "eslint",
    "intellij",
    "nosonar",
    "pmd",
    "spotbugs",
    "suppress-warnings-all",
];

/// The sources `compat: true` selects: every source honored before sources could be chosen.
pub const COMPAT_TRUE: [&str; 5] = [
    "checkstyle",
    "eslint",
    "nosonar",
    "pmd",
    "suppress-warnings-all",
];

/// Kind prefix of a plugin's compatibility suppressions: `compat:pmd`.
pub const COMPAT_KIND_PREFIX: &str = "compat:";

/// A set of [`COMPAT_SOURCES`] (a bit per source, in the table's order).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CompatSet(u16);

impl CompatSet {
    /// No source.
    pub const NONE: CompatSet = CompatSet(0);

    /// The named sources; unknown names are ignored (the configuration is validated before).
    pub fn of<'a>(names: impl IntoIterator<Item = &'a str>) -> CompatSet {
        let mut bits = 0;
        for n in names {
            if let Some(i) = COMPAT_SOURCES.iter().position(|s| *s == n.trim()) {
                bits |= 1 << i;
            }
        }
        CompatSet(bits)
    }

    /// What `compat: true` selects ([`COMPAT_TRUE`]).
    pub fn legacy() -> CompatSet {
        CompatSet::of(COMPAT_TRUE)
    }

    /// Whether `source` is selected.
    pub fn contains(self, source: &str) -> bool {
        COMPAT_SOURCES
            .iter()
            .position(|s| *s == source)
            .is_some_and(|i| self.0 & (1 << i) != 0)
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The bits (for hashing).
    pub fn bits(self) -> u16 {
        self.0
    }

    /// Whether a plugin suppression of `kind` applies: always for kinds that are not
    /// compatibility kinds; for `compat:<source>` when the source is selected; for the untagged
    /// `compat` when any source is.
    pub fn honors(self, kind: &str) -> bool {
        match kind.strip_prefix(COMPAT_KIND_PREFIX) {
            Some(source) => self.contains(source),
            None if kind == "compat" => !self.is_empty(),
            None => true,
        }
    }
}

/// Where a suppression came from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SuppressionSource {
    /// `mote-ignore[...]` / `mote-ignore:` comment.
    Comment,
    /// `mote-ignore-file[...]` comment.
    FileComment,
    /// `NOSONAR` comment (compat mode).
    NoSonar,
    /// Plugin-reported suppression; the string is the plugin's kind.
    Scoped(String),
}

/// A suppression parsed for one file. Stored in the per-file cache entry so that findings
/// produced later (duplication) can be suppressed and unused suppressions reported.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Suppression {
    pub source: SuppressionSource,
    /// First line covered.
    pub start_line: u32,
    /// Last line covered (inclusive).
    pub end_line: u32,
    /// Full rule ids; `None` = every rule.
    pub rules: Option<Vec<String>>,
    pub justification: Option<String>,
    /// Location of the directive itself (for meta findings).
    pub line: u32,
    pub col: u32,
    /// Whitespace-collapsed text of the directive's line (fingerprint anchor).
    pub anchor: String,
    /// Whether it suppressed at least one finding.
    pub used: bool,
}

impl Suppression {
    /// The kind recorded on suppressed findings (`compat` for every compatibility source but
    /// `NOSONAR`).
    pub fn kind(&self) -> &str {
        match &self.source {
            SuppressionSource::Comment => "mote-ignore",
            SuppressionSource::FileComment => "mote-ignore-file",
            SuppressionSource::NoSonar => "nosonar",
            SuppressionSource::Scoped(k) if k.starts_with(COMPAT_KIND_PREFIX) => "compat",
            SuppressionSource::Scoped(k) => k,
        }
    }

    /// Whether this suppression covers `rule_id` at `line`.
    pub fn covers(&self, rule_id: &str, line: u32) -> bool {
        line >= self.start_line
            && line <= self.end_line
            && self
                .rules
                .as_ref()
                .is_none_or(|r| r.iter().any(|x| x == rule_id))
    }

    /// Engine-owned comment directives are checked for being unused.
    pub fn checks_unused(&self) -> bool {
        matches!(
            self.source,
            SuppressionSource::Comment | SuppressionSource::FileComment
        )
    }

    fn info(&self) -> SuppressionInfo {
        SuppressionInfo {
            kind: self.kind().to_string(),
            justification: self.justification.clone(),
        }
    }
}

fn normalize_rule_ref(r: &str, language: &str) -> String {
    let r = r.trim();
    if r.contains('/') {
        r.to_string()
    } else {
        format!("{language}/{r}")
    }
}

fn parse_reason(rest: &str) -> Option<String> {
    let rest = rest.trim_start();
    let rest = rest.strip_prefix(':')?;
    let rest = rest.split("*/").next().unwrap_or("");
    let r = rest.trim();
    (!r.is_empty()).then(|| r.to_string())
}

/// Whether the trimmed line `t` holds only annotations / decorators (`@Deprecated`,
/// `@SuppressWarnings("x")`), or the continuation of one whose arguments span lines (`depth`
/// counts the parentheses still open).
fn only_annotations(t: &str, depth: &mut u32) -> bool {
    let b = t.as_bytes();
    let mut i = 0;
    loop {
        while *depth > 0 && i < b.len() {
            match b[i] {
                b'(' => *depth += 1,
                b')' => *depth -= 1,
                _ => {}
            }
            i += 1;
        }
        if *depth > 0 {
            return true;
        }
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let rest = &b[i..];
        if rest.is_empty() || rest.starts_with(b"//") || rest.starts_with(b"/*") {
            return true;
        }
        if rest[0] != b'@' {
            return false;
        }
        i += 1;
        while i < b.len() && (b[i].is_ascii_alphanumeric() || matches!(b[i], b'_' | b'.' | b'$')) {
            i += 1;
        }
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i < b.len() && b[i] == b'(' {
            *depth += 1;
            i += 1;
        }
    }
}

/// Lines a standalone directive ending on `after` applies to: the next code line and, when that
/// line starts a run of annotation lines, everything up to the declaration they annotate (where
/// class and method findings are reported).
fn target_lines(lex: &LexOutput, index: &LineIndex<'_>, after: u32) -> Option<(u32, u32)> {
    let first = lex.next_code_line_after(after)?;
    let mut last = first;
    let mut depth = 0u32;
    for _ in 0..64 {
        if !only_annotations(index.line_text(last).trim(), &mut depth) {
            break;
        }
        match lex.next_code_line_after(last) {
            Some(next) => last = next,
            None => break,
        }
    }
    Some((first, last))
}

/// Whether `text[at..]` starts a word: the character before it (if any) is not part of one.
/// Keeps the engine's `mote-` directives from matching inside `remote-…` or `promote-…`.
pub(crate) fn starts_word(text: &str, at: usize) -> bool {
    !text[..at]
        .chars()
        .next_back()
        .is_some_and(|ch| ch.is_alphanumeric() || ch == '_' || ch == '-')
}

/// Parse all suppressions of a file; `compat` selects the other tools' markers honored.
pub fn collect(
    comments: &[Comment],
    scoped: &[ScopedSuppression],
    language: &str,
    compat: CompatSet,
    lex: &LexOutput,
    index: &LineIndex<'_>,
) -> Vec<Suppression> {
    let nosonar = compat.contains("nosonar");
    const DIRECTIVE: &str = "mote-ignore";
    let mut out = Vec::new();
    for c in comments {
        for (i, text) in c.text.split('\n').enumerate() {
            let line = c.start_line + i as u32;
            let col_base = if i == 0 { c.start_col } else { 1 };
            let mut from = 0;
            while let Some(pos) = text[from..].find(DIRECTIVE) {
                let at = from + pos;
                from = at + DIRECTIVE.len();
                // Must be the whole word: not the tail of another one (`remote-ignore`) ...
                if !starts_word(text, at) {
                    continue;
                }
                let mut rest = &text[from..];
                let file = rest.starts_with("-file");
                if file {
                    rest = &rest["-file".len()..];
                }
                // ... and followed by `[`, `:`, whitespace, `*` or the end.
                if !(rest.is_empty() || rest.starts_with(['[', ':', ' ', '\t', '*', '\r'])) {
                    continue;
                }
                let mut rules = None;
                if let Some(list) = rest.strip_prefix('[') {
                    let Some(close) = list.find(']') else {
                        continue;
                    };
                    let ids: Vec<String> = list[..close]
                        .split(',')
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .map(|s| normalize_rule_ref(s, language))
                        .collect();
                    rest = &list[close + 1..];
                    rules = Some(ids);
                }
                let justification = parse_reason(rest);
                let (start_line, end_line) = if file {
                    (1, u32::MAX)
                } else if c.trailing && i == 0 {
                    (line, line)
                } else {
                    target_lines(lex, index, c.end_line).unwrap_or((line, line))
                };
                out.push(Suppression {
                    source: if file {
                        SuppressionSource::FileComment
                    } else {
                        SuppressionSource::Comment
                    },
                    start_line,
                    end_line,
                    rules,
                    justification,
                    line,
                    col: col_base + text[..at].chars().count() as u32,
                    anchor: anchor_text(index.line_text(line)),
                    used: false,
                });
            }
            if nosonar {
                if let Some(pos) = text.find("NOSONAR") {
                    out.push(Suppression {
                        source: SuppressionSource::NoSonar,
                        start_line: line,
                        end_line: line,
                        rules: None,
                        justification: None,
                        line,
                        col: col_base + text[..pos].chars().count() as u32,
                        anchor: anchor_text(index.line_text(line)),
                        used: false,
                    });
                }
            }
        }
    }
    for s in scoped {
        if !compat.honors(&s.kind) {
            continue;
        }
        out.push(Suppression {
            source: SuppressionSource::Scoped(s.kind.clone()),
            start_line: s.start_line,
            end_line: s.end_line.max(s.start_line),
            rules: s
                .rules
                .as_ref()
                .map(|r| r.iter().map(|x| normalize_rule_ref(x, language)).collect()),
            justification: s.justification.clone(),
            line: s.start_line,
            col: 1,
            anchor: anchor_text(index.line_text(s.start_line)),
            used: false,
        });
    }
    out
}

/// Find the suppression for a finding (and mark every matching suppression used).
/// Meta rule findings are never suppressed.
pub fn suppress(rule_id: &str, line: u32, sups: &mut [Suppression]) -> Option<SuppressionInfo> {
    if rule_id.starts_with("mote/") {
        return None;
    }
    let mut info = None;
    for s in sups.iter_mut() {
        if s.covers(rule_id, line) {
            s.used = true;
            if info.is_none() {
                info = Some(s.info());
            }
        }
    }
    info
}

/// Suppressions indexed by line, so that [`SuppressionIndex::suppress`] for every finding of a
/// file does not scan every suppression per finding (quadratic in a file with thousands of
/// directives and findings). Suppressions spanning up to [`NARROW_SPAN`] lines are listed under
/// each of their lines; wider ones (file-level directives, regions, annotated declarations) are
/// checked for every finding.
pub struct SuppressionIndex {
    by_line: rustc_hash::FxHashMap<u32, Vec<u32>>,
    wide: Vec<u32>,
}

/// Widest suppression (in lines, minus one) [`SuppressionIndex`] lists per line.
const NARROW_SPAN: u32 = 8;

impl SuppressionIndex {
    pub fn new(sups: &[Suppression]) -> Self {
        let mut by_line: rustc_hash::FxHashMap<u32, Vec<u32>> = Default::default();
        let mut wide = Vec::new();
        for (i, s) in sups.iter().enumerate() {
            if s.end_line >= s.start_line && s.end_line - s.start_line <= NARROW_SPAN {
                for line in s.start_line..=s.end_line {
                    by_line.entry(line).or_default().push(i as u32);
                }
            } else {
                wide.push(i as u32);
            }
        }
        SuppressionIndex { by_line, wide }
    }

    /// [`suppress`] for the suppressions this index was built from.
    pub fn suppress(
        &self,
        rule_id: &str,
        line: u32,
        sups: &mut [Suppression],
    ) -> Option<SuppressionInfo> {
        if rule_id.starts_with("mote/") {
            return None;
        }
        let narrow = self.by_line.get(&line).map_or(&[][..], Vec::as_slice);
        // The info of the first matching suppression in `sups` order, as `suppress` does.
        let mut first: Option<u32> = None;
        for &i in narrow.iter().chain(&self.wide) {
            if let Some(s) = sups.get_mut(i as usize) {
                if s.covers(rule_id, line) {
                    s.used = true;
                    first = Some(first.map_or(i, |f| f.min(i)));
                }
            }
        }
        first.map(|i| sups[i as usize].info())
    }
}

fn meta_finding(rule: MetaRule, s: &Suppression, message: &str) -> RawFinding {
    let len = match s.source {
        SuppressionSource::NoSonar => "NOSONAR".len(),
        SuppressionSource::FileComment => "mote-ignore-file".len(),
        _ => "mote-ignore".len(),
    } as u32;
    RawFinding {
        rule_id: rule.id().to_string(),
        message: message.to_string(),
        range: TextRange {
            start_line: s.line,
            start_col: s.col,
            end_line: s.line,
            end_col: s.col + len,
        },
        symbol: None,
        value: None,
        secondary: Vec::new(),
    }
}

/// Blanket and missing-reason findings (known right after parsing).
pub fn directive_findings(
    sups: &[Suppression],
    rules: &RuleSet,
    require_reason: bool,
) -> Vec<RawFinding> {
    let mut out = Vec::new();
    let blanket_on = rules.is_enabled(MetaRule::BlanketSuppression.id());
    let reason_on = require_reason && rules.is_enabled(MetaRule::SuppressionWithoutReason.id());
    for s in sups {
        match s.source {
            SuppressionSource::Comment | SuppressionSource::FileComment => {
                if blanket_on && s.rules.is_none() {
                    out.push(meta_finding(
                        MetaRule::BlanketSuppression,
                        s,
                        "List the rules this suppression is meant for instead of silencing every rule",
                    ));
                }
                if reason_on && s.justification.is_none() {
                    out.push(meta_finding(
                        MetaRule::SuppressionWithoutReason,
                        s,
                        "Explain why the finding is suppressed after `mote-ignore[...]:`",
                    ));
                }
            }
            SuppressionSource::NoSonar => {
                if blanket_on {
                    out.push(meta_finding(
                        MetaRule::BlanketSuppression,
                        s,
                        "Replace this NOSONAR with `mote-ignore[<rule>]: <reason>` naming the suppressed rule",
                    ));
                }
            }
            SuppressionSource::Scoped(_) => {}
        }
    }
    out
}

/// Findings for comment directives that suppressed nothing.
pub fn unused_findings(sups: &[Suppression], rules: &RuleSet) -> Vec<(RawFinding, String)> {
    if !rules.is_enabled(MetaRule::UnusedSuppression.id()) {
        return Vec::new();
    }
    sups.iter()
        .filter(|s| s.checks_unused() && !s.used)
        .map(|s| {
            let what = match &s.rules {
                Some(r) if !r.is_empty() => format!(
                    " for {}",
                    r.iter()
                        .map(|x| format!("`{x}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                _ => String::new(),
            };
            (
                meta_finding(
                    MetaRule::UnusedSuppression,
                    s,
                    &format!(
                        "Remove this unused suppression{what}; it does not suppress any finding"
                    ),
                ),
                s.anchor.clone(),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_agrees_with_linear_suppress() {
        let mk = |start: u32, end: u32, rules: Option<&[&str]>, j: Option<&str>| Suppression {
            source: SuppressionSource::Comment,
            start_line: start,
            end_line: end,
            rules: rules.map(|r| r.iter().map(|s| s.to_string()).collect()),
            justification: j.map(str::to_string),
            line: start,
            col: 1,
            anchor: String::new(),
            used: false,
        };
        let base = vec![
            mk(1, u32::MAX, Some(&["x/a"]), Some("file")),
            mk(3, 3, None, Some("blanket")),
            mk(3, 3, Some(&["x/b"]), Some("narrow b")),
            mk(2, 40, Some(&["x/b", "x/c"]), Some("wide")),
            mk(5, 12, Some(&["x/c"]), Some("span 8")),
            mk(7, 6, None, Some("inverted")),
            mk(41, 41, Some(&["x/a"]), None),
        ];
        let mut linear = base.clone();
        let mut indexed = base.clone();
        let index = SuppressionIndex::new(&indexed);
        for line in 0..45 {
            for rule in ["x/a", "x/b", "x/c", "x/d", "mote/parse-error"] {
                assert_eq!(
                    suppress(rule, line, &mut linear),
                    index.suppress(rule, line, &mut indexed),
                    "{rule} at {line}"
                );
            }
        }
        let used = |v: &[Suppression]| v.iter().map(|s| s.used).collect::<Vec<_>>();
        assert_eq!(used(&linear), used(&indexed));
    }
    use crate::lexer::{lex, LexOptions};
    use mote_core::{ActiveRule, LexerSpec, Severity};
    use rustc_hash::FxHashMap;
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

    fn parse(text: &str, compat: CompatSet, scoped: &[ScopedSuppression]) -> Vec<Suppression> {
        let lexed = lex(text, &spec(), LexOptions::default());
        let idx = LineIndex::new(text);
        collect(&lexed.comments, scoped, "java", compat, &lexed, &idx)
    }

    fn all_meta() -> RuleSet {
        let mut m = FxHashMap::default();
        for r in MetaRule::ALL {
            m.insert(
                r.id().to_string(),
                ActiveRule {
                    severity: Severity::Low,
                    params: BTreeMap::new(),
                },
            );
        }
        RuleSet::new(m)
    }

    #[test]
    fn parses_directives_and_targets() {
        let text = "\
// mote-ignore[empty-catch-block, js/x]: legacy code
/* spacer */

foo(); // mote-ignore[magic-number]
bar();
// mote-ignore: whatever
baz();
// mote-ignore-file[java/todo-comment]: generated
// mote-ignored is not a directive
// remote-ignore[java/x]: nor is this
";
        let s = parse(text, CompatSet::NONE, &[]);
        assert_eq!(s.len(), 4, "{s:#?}");
        assert_eq!(
            s[0].rules,
            Some(vec![
                "java/empty-catch-block".to_string(),
                "js/x".to_string()
            ])
        );
        assert_eq!(s[0].justification.as_deref(), Some("legacy code"));
        assert_eq!(
            (s[0].start_line, s[0].end_line),
            (4, 4),
            "next code line, skipping comments and blanks"
        );
        assert_eq!(
            (s[1].start_line, s[1].justification.as_deref()),
            (4, None),
            "trailing applies to own line"
        );
        assert_eq!(s[1].col, 11);
        assert_eq!(s[2].rules, None);
        assert_eq!(s[2].start_line, 7);
        assert_eq!(s[3].source, SuppressionSource::FileComment);
        assert_eq!((s[3].start_line, s[3].end_line), (1, u32::MAX));
    }

    #[test]
    fn standalone_directive_covers_annotations_and_the_declaration() {
        let text = "\
class A {
    // mote-ignore[java/x]: legacy API
    @Deprecated
    @SuppressWarnings({
        \"unchecked\",
    })
    public void old() {}

    // mote-ignore[java/y]: inline annotation
    @Override public void a() {}
    void b() {}

    // mote-ignore[java/z]: trailing comment on the annotation
    @Test // flaky
    void c() {}

    // mote-ignore[java/w]: not an annotation
    void d() {}
    void e() {}
}
";
        let s = parse(text, CompatSet::NONE, &[]);
        let ranges: Vec<(u32, u32)> = s.iter().map(|s| (s.start_line, s.end_line)).collect();
        assert_eq!(ranges, vec![(3, 7), (10, 10), (14, 15), (18, 18)], "{s:#?}");
        let mut s = s;
        assert!(suppress("java/x", 7, &mut s).is_some(), "declaration line");
        assert!(suppress("java/x", 8, &mut s).is_none());
        assert!(suppress("java/y", 11, &mut s).is_none());
        assert!(unused_findings(&s, &all_meta())
            .iter()
            .all(|(f, _)| f.range.start_line != 2));
    }

    #[test]
    fn nosonar_and_scoped_depend_on_compat() {
        let text = "a(); // NOSONAR\n";
        let scoped = [
            ScopedSuppression {
                start_line: 1,
                end_line: 3,
                rules: None,
                kind: "compat".into(),
                justification: None,
            },
            ScopedSuppression {
                start_line: 1,
                end_line: 3,
                rules: Some(vec!["magic".into()]),
                kind: "annotation".into(),
                justification: Some("why".into()),
            },
        ];
        let off = parse(text, CompatSet::NONE, &scoped);
        assert_eq!(off.len(), 1);
        assert_eq!(off[0].rules, Some(vec!["java/magic".to_string()]));
        let on = parse(text, CompatSet::legacy(), &scoped);
        assert_eq!(on.len(), 3);
        assert!(on.iter().any(|s| s.source == SuppressionSource::NoSonar));
    }

    #[test]
    fn compat_sources_are_selected_one_by_one() {
        let text = "a(); // NOSONAR\nb();\nc();\nd();\n";
        let tagged = |line: u32, source: &str| ScopedSuppression {
            start_line: line,
            end_line: line,
            rules: None,
            kind: format!("{COMPAT_KIND_PREFIX}{source}"),
            justification: None,
        };
        let scoped = [
            tagged(2, "pmd"),
            tagged(3, "spotbugs"),
            tagged(4, "intellij"),
            tagged(4, "a-future-tool"),
        ];
        let lines = |compat: CompatSet| -> Vec<(u32, String)> {
            parse(text, compat, &scoped)
                .iter()
                .map(|s| (s.start_line, s.kind().to_string()))
                .collect()
        };
        // `true`: exactly the sources honored before (no spotbugs, no intellij).
        assert_eq!(
            lines(CompatSet::legacy()),
            vec![(1, "nosonar".into()), (2, "compat".into())]
        );
        assert_eq!(lines(CompatSet::NONE), vec![]);
        // A mixed list: only those sources, NOSONAR included only when listed.
        assert_eq!(
            lines(CompatSet::of(["spotbugs", "intellij"])),
            vec![(3, "compat".into()), (4, "compat".into())]
        );
        assert_eq!(
            lines(CompatSet::of(["nosonar", "pmd"])),
            vec![(1, "nosonar".into()), (2, "compat".into())]
        );
        // Unknown names select nothing.
        assert_eq!(
            lines(CompatSet::of(["a-future-tool", " pmd "])),
            vec![(2, "compat".into())]
        );

        let legacy = CompatSet::legacy();
        for s in COMPAT_TRUE {
            assert!(legacy.contains(s), "{s}");
        }
        assert!(!legacy.contains("spotbugs") && !legacy.contains("intellij"));
        assert!(COMPAT_SOURCES.windows(2).all(|w| w[0] < w[1]), "sorted");
        assert!(legacy.honors("annotation") && CompatSet::NONE.honors("annotation"));
        assert!(legacy.honors("compat") && !CompatSet::NONE.honors("compat"));
    }

    #[test]
    fn suppression_application_and_meta_findings() {
        let text = "\
foo(); // mote-ignore[java/a]: ok
bar(); // mote-ignore[java/b]
baz(); // mote-ignore
qux(); // NOSONAR
";
        let mut s = parse(text, CompatSet::legacy(), &[]);
        assert_eq!(suppress("java/a", 1, &mut s).unwrap().kind, "mote-ignore");
        assert!(suppress("java/a", 2, &mut s).is_none());
        assert!(suppress("java/zzz", 3, &mut s).is_some(), "blanket");
        assert_eq!(suppress("java/zzz", 4, &mut s).unwrap().kind, "nosonar");
        assert!(
            suppress("mote/parse-error", 3, &mut s).is_none(),
            "meta rules are not suppressible"
        );
        let rules = all_meta();
        let d = directive_findings(&s, &rules, true);
        let ids: Vec<(&str, u32)> = d
            .iter()
            .map(|f| (f.rule_id.as_str(), f.range.start_line))
            .collect();
        assert_eq!(
            ids,
            vec![
                ("mote/suppression-without-reason", 2),
                ("mote/blanket-suppression", 3),
                ("mote/suppression-without-reason", 3),
                ("mote/blanket-suppression", 4),
            ]
        );
        let d = directive_findings(&s, &rules, false);
        assert_eq!(d.len(), 2);
        let unused = unused_findings(&s, &rules);
        assert_eq!(unused.len(), 1);
        assert_eq!(unused[0].0.range.start_line, 2);
        assert!(unused[0].0.message.contains("`java/b`"));
        assert_eq!(unused[0].1, "bar(); // mote-ignore[java/b]");
    }
}
