//! Mote output formats.
//!
//! Every format is a pure function of a [`ScanResult`] (plus options) that writes bytes:
//!
//! | Module | Format |
//! |---|---|
//! | [`text`] | human-readable, grouped by file, optional ANSI colors, summary and gate verdict |
//! | [`json`] | the complete `ScanResult` (input for `mote gate --results`) and JSONL |
//! | [`sarif`] | SARIF 2.1.0 for code-scanning tools |
//! | [`markdown`] | job summary / sticky PR comment (starts with [`MARKER`]) |
//! | [`github`] | GitHub Checks API payload with annotations |
//! | [`gitlab`] | GitLab Code Quality report (Code Climate issue subset) |
//! | [`checkstyle`] | Checkstyle XML (CI dashboards such as Jenkins Warnings NG) |
//! | [`junit`] | JUnit XML: a test suite for the gate, one per rule (CI test views) |
//! | [`html`] | a self-contained HTML report (overview, directory tree, rules) |
//! | [`bitbucket`] | Bitbucket Code Insights report and annotations |
//! | [`summary`] | a one-line metrics record for trend archives |
//! | [`compare`] | the differences between two saved results (text, Markdown, JSON) |
//! | [`diff`] | unified diffs of the files `mote fix` changes |
//! | [`rules`] | the rule catalog (text table, JSON, Markdown) |
//! | [`precision`] | measured rule precision: data types and how the catalog shows it |
//!
//! Nothing here reads the file system, the clock, or the environment: the same result always
//! renders to the same bytes (the only non-deterministic input is `ScanResult::stats`, which only
//! the text summary and the JSON dump show).

pub mod bitbucket;
pub mod checkstyle;
pub mod compare;
pub mod diff;
pub mod github;
pub mod gitlab;
pub mod html;
pub mod json;
pub mod junit;
pub mod markdown;
pub mod precision;
pub mod rules;
pub mod sarif;
mod sha256;
pub mod summary;
pub mod text;
pub mod xml;

#[cfg(test)]
mod testdata;

use mote_core::{Finding, FindingStatus, Severity};
use mote_engine::ScanResult;

/// Hidden marker on the first line of the Markdown summary (the GitHub Action finds its sticky PR
/// comment by it).
pub const MARKER: &str = "<!-- mote:summary -->";

/// Version of the reporting tool (the workspace version).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Placeholder project URL used in SARIF `informationUri`.
pub const INFORMATION_URI: &str = "https://github.com/atmuccio/mote";

/// Which findings the human/agent-facing formats (text, JSONL, Markdown) show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Filter {
    /// Only findings that are new or worsened relative to the base (everything when there is no
    /// base, because then every finding counts as new code).
    pub new_only: bool,
    /// Hide findings below this severity.
    pub min_severity: Severity,
    /// Include suppressed findings.
    pub show_suppressed: bool,
}

impl Default for Filter {
    fn default() -> Self {
        Filter {
            new_only: false,
            min_severity: Severity::Low,
            show_suppressed: false,
        }
    }
}

impl Filter {
    /// Whether `f` passes the filter.
    pub fn accepts(&self, f: &Finding) -> bool {
        (self.show_suppressed || f.suppressed.is_none())
            && f.severity >= self.min_severity
            && (!self.new_only || is_new_like(f))
    }

    /// The findings of `result` that pass, in result order.
    pub fn apply<'a>(&'a self, result: &'a ScanResult) -> impl Iterator<Item = &'a Finding> + 'a {
        result.findings.iter().filter(move |f| self.accepts(f))
    }
}

/// New, worsened, or unknown (no base: everything is "new code").
pub fn is_new_like(f: &Finding) -> bool {
    matches!(
        f.status,
        FindingStatus::New | FindingStatus::Worsened | FindingStatus::Unknown
    )
}

/// Whether the result was compared with a base.
pub fn has_base(result: &ScanResult) -> bool {
    result.new_code.is_some()
}

/// Parse `HIGH` / `medium` / `Low`.
pub fn parse_severity(s: &str) -> Option<Severity> {
    match s.to_ascii_uppercase().as_str() {
        "HIGH" => Some(Severity::High),
        "MEDIUM" => Some(Severity::Medium),
        "LOW" => Some(Severity::Low),
        _ => None,
    }
}

/// Lower-case status name as serialized (`new`, `unchanged`, `worsened`, `unknown`).
pub fn status_str(s: FindingStatus) -> &'static str {
    match s {
        FindingStatus::Unknown => "unknown",
        FindingStatus::New => "new",
        FindingStatus::Unchanged => "unchanged",
        FindingStatus::Worsened => "worsened",
    }
}

/// Human label for a gate condition.
pub fn condition_label(name: &str) -> String {
    match name {
        "new_findings" => "New findings".into(),
        "findings" => "Findings".into(),
        "new_duplication_pct" => "Duplication on new code".into(),
        "new_coverage_pct" => "Coverage on new code".into(),
        "worsened" => "Worsened findings".into(),
        "maintainability_grade" => "Maintainability grade".into(),
        "reliability_grade" => "Reliability grade".into(),
        "new_maintainability_grade" => "Maintainability grade on new code".into(),
        "new_reliability_grade" => "Reliability grade on new code".into(),
        other => other.replace('_', " "),
    }
}

/// Whether a condition compares grades (its values are ranks: A = 1 … E = 5).
pub fn condition_is_grade(name: &str) -> bool {
    name.ends_with("_grade")
}

/// Whether a condition's value is a percentage.
pub fn condition_is_pct(name: &str) -> bool {
    name.ends_with("_pct")
}

/// The comparison a condition checks, e.g. `<=` for maxima (grades: the rank is at most the
/// threshold's, i.e. no worse).
pub fn condition_op(name: &str) -> &'static str {
    if name == "new_coverage_pct" {
        ">="
    } else {
        "<="
    }
}

/// A condition's threshold with its comparison: `<= 0`, `>= 80.0%`, `B or better`.
pub fn fmt_condition_threshold(name: &str, v: f64) -> String {
    if condition_is_grade(name) {
        format!("{} or better", fmt_condition_value(name, v))
    } else {
        format!("{} {}", condition_op(name), fmt_condition_value(name, v))
    }
}

/// Format a condition value: `12`, `3.5%`, or a grade letter.
pub fn fmt_condition_value(name: &str, v: f64) -> String {
    if condition_is_grade(name) {
        let rank = if (1.0..=5.0).contains(&v) { v as u8 } else { 0 };
        match mote_engine::grades::Grade::from_rank(rank) {
            Some(g) => g.as_str().to_string(),
            None => format!("{v}"),
        }
    } else if condition_is_pct(name) {
        format!("{v:.1}%")
    } else if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// `1234567` → `1,234,567`.
pub fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Estimated effort (minutes) in hours: `0 h`, `< 0.1 h`, `0.8 h`, `12 h`, `1,234 h`.
pub fn fmt_hours(minutes: u64) -> String {
    let h = minutes as f64 / 60.0;
    if minutes == 0 {
        "0 h".into()
    } else if minutes < 6 {
        "< 0.1 h".into()
    } else if h < 9.95 {
        format!("{h:.1} h")
    } else {
        format!("{} h", thousands(h.round() as u64))
    }
}

/// Milliseconds as `850 ms` or `1.25 s`.
pub fn fmt_ms(ms: f64) -> String {
    if ms < 1000.0 {
        format!("{} ms", ms.round() as u64)
    } else {
        format!("{:.2} s", ms / 1000.0)
    }
}

/// Truncate `s` to at most `max` bytes on a char boundary.
pub fn truncate_bytes(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Short commit (12 hex digits).
pub fn short_commit(c: &str) -> &str {
    truncate_bytes(c, 12)
}

/// Rule id of the engine's "file could not be analyzed" finding.
pub const INTERNAL_ERROR_RULE: &str = "mote/internal-error";

/// Files that could not be analyzed: files with an internal-error finding, plus error
/// diagnostics. Non-zero means the scan is incomplete (`--strict` exits 3; SARIF reports
/// `executionSuccessful: false`).
pub fn failed_files(result: &ScanResult) -> usize {
    let files: std::collections::BTreeSet<&str> = result
        .findings
        .iter()
        .filter(|f| f.rule_id == INTERNAL_ERROR_RULE)
        .map(|f| f.path.as_str())
        .collect();
    files.len()
        + result
            .diagnostics
            .iter()
            .filter(|d| d.level == mote_engine::result::DiagnosticLevel::Error)
            .count()
}

/// `prefix` + `path`: a scan-root-relative path made relative to the repository top level.
///
/// Text, JSON and JSONL keep paths relative to the scan root (what a local user or agent works
/// with). GitHub annotations, SARIF and the Markdown summary are resolved by GitHub from the
/// repository root, so when the scan root is a subdirectory (`mote scan backend`) their paths
/// get the root's repository-relative prefix (`backend/`).
pub fn repo_path<'a>(prefix: &str, path: &'a str) -> std::borrow::Cow<'a, str> {
    if prefix.is_empty() || path.is_empty() {
        std::borrow::Cow::Borrowed(path)
    } else if prefix.ends_with('/') {
        std::borrow::Cow::Owned(format!("{prefix}{path}"))
    } else {
        std::borrow::Cow::Owned(format!("{prefix}/{path}"))
    }
}

/// At most `max` of `list` (in its order): the most severe, ties kept in list order. The SARIF,
/// JUnit and HTML caps use this rule.
pub fn most_severe(list: Vec<&Finding>, max: usize) -> Vec<&Finding> {
    if list.len() <= max {
        return list;
    }
    let mut indexed: Vec<(usize, &Finding)> = list.into_iter().enumerate().collect();
    indexed.sort_by(|a, b| b.1.severity.cmp(&a.1.severity).then(a.0.cmp(&b.0)));
    indexed.truncate(max);
    indexed.sort_by_key(|&(i, _)| i);
    indexed.into_iter().map(|(_, f)| f).collect()
}

/// Findings sorted for "top N" lists: severity (high first), then the result order.
pub fn by_severity<'a>(findings: impl Iterator<Item = &'a Finding>) -> Vec<&'a Finding> {
    let mut v: Vec<&Finding> = findings.collect();
    v.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then_with(|| a.sort_key().cmp(&b.sort_key()))
    });
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1000), "1,000");
        assert_eq!(thousands(1234567), "1,234,567");
        assert_eq!(fmt_ms(12.4), "12 ms");
        assert_eq!(fmt_ms(1250.0), "1.25 s");
        assert_eq!(fmt_hours(0), "0 h");
        assert_eq!(fmt_hours(5), "< 0.1 h");
        assert_eq!(fmt_hours(45), "0.8 h");
        assert_eq!(fmt_hours(599), "10 h");
        assert_eq!(fmt_hours(74_040), "1,234 h");
        assert_eq!(truncate_bytes("héllo", 2), "h");
        assert_eq!(parse_severity("medium"), Some(Severity::Medium));
        assert_eq!(parse_severity("x"), None);
        assert_eq!(fmt_condition_value("new_duplication_pct", 3.0), "3.0%");
        assert_eq!(fmt_condition_value("new_findings", 2.0), "2");
        assert_eq!(fmt_condition_value("new_maintainability_grade", 2.0), "B");
        assert_eq!(fmt_condition_value("reliability_grade", 9.0), "9");
        assert_eq!(
            fmt_condition_threshold("reliability_grade", 1.0),
            "A or better"
        );
        assert_eq!(fmt_condition_threshold("new_findings", 0.0), "<= 0");
        assert_eq!(
            fmt_condition_threshold("new_coverage_pct", 80.0),
            ">= 80.0%"
        );
        assert_eq!(
            condition_label("new_reliability_grade"),
            "Reliability grade on new code"
        );
        assert_eq!(repo_path("", "src/A.java"), "src/A.java");
        assert_eq!(repo_path("backend/", "src/A.java"), "backend/src/A.java");
        assert_eq!(repo_path("backend", "src/A.java"), "backend/src/A.java");
    }

    #[test]
    fn failed_files_counts_internal_errors_and_error_diagnostics() {
        let mut r = testdata::without_base();
        assert_eq!(failed_files(&r), 0, "warnings don't count");
        let mut f = r.findings[0].clone();
        f.rule_id = INTERNAL_ERROR_RULE.into();
        r.findings.push(f.clone());
        r.findings.push(f);
        r.diagnostics.push(mote_engine::result::Diagnostic {
            level: mote_engine::result::DiagnosticLevel::Error,
            path: Some("x.java".into()),
            message: "unreadable".into(),
        });
        assert_eq!(
            failed_files(&r),
            2,
            "one file with two errors + one diagnostic"
        );
    }
}
