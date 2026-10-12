//! `mote compare old.json new.json`: what changed between two saved results.
//!
//! Findings are paired with [`mote_engine::matching`] (path + fingerprint, then fingerprint
//! across a rename or move, then measured findings of the same symbol) and sorted into:
//!
//! * **added** — unsuppressed in the new result, no counterpart in the old one;
//! * **fixed** — unsuppressed in the old result, no counterpart in the new one;
//! * **severity changed** — paired, unsuppressed on both sides, another severity;
//! * **newly suppressed** / **unsuppressed** — paired, suppressed on one side only.
//!
//! Measures and grades are compared too. Differences that make findings look added or fixed
//! although the code did not change — another engine version, results format or rule set — are
//! reported as warnings. The comparison is a pure function of the two results.

use crate::{fmt_hours, short_commit, thousands};
use mote_core::{Finding, Severity};
use mote_engine::baseline::rule_set_difference;
use mote_engine::grades::{self, Grades};
use mote_engine::matching::{self, Files};
use mote_engine::ScanResult;
use rustc_hash::FxHashSet;
use serde::Serialize;
use serde_json::{json, Value};
use std::fmt::Write as _;

/// Version of the comparison JSON (`compare --format json`).
pub const COMPARE_VERSION: u32 = 1;

/// One side of a comparison.
#[derive(Clone, Debug, Serialize)]
pub struct Side {
    /// The results file as given.
    pub file: String,
    pub engine_version: String,
    pub format_version: u32,
    pub kind: String,
    pub rev: Option<String>,
    pub commit: Option<String>,
    /// Unsuppressed findings.
    pub findings: u64,
    pub gate_passed: bool,
}

/// A finding in a comparison list.
#[derive(Clone, Debug, Serialize)]
pub struct Item {
    pub id: String,
    pub rule: String,
    pub severity: Severity,
    /// The severity in the old result (severity changes only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_severity: Option<Severity>,
    pub path: String,
    /// The path in the old result, when the file was renamed or moved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_path: Option<String>,
    pub line: u32,
    pub message: String,
    pub fingerprint: String,
}

impl Item {
    fn of(f: &Finding) -> Item {
        Item {
            id: f.id.clone(),
            rule: f.rule_id.clone(),
            severity: f.severity,
            old_severity: None,
            path: f.path.clone(),
            old_path: None,
            line: f.range.start_line,
            message: f.message.clone(),
            fingerprint: f.fingerprint.clone(),
        }
    }
}

/// Counts of each list.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Counts {
    pub added: u64,
    pub fixed: u64,
    pub severity_changed: u64,
    pub newly_suppressed: u64,
    pub unsuppressed: u64,
    /// Paired, unsuppressed on both sides, same severity.
    pub unchanged: u64,
}

/// A measure on both sides.
#[derive(Clone, Debug, Serialize)]
pub struct MeasureDelta {
    pub name: &'static str,
    pub label: &'static str,
    pub old: Option<f64>,
    pub new: Option<f64>,
    /// `new - old` (rounded to 2 decimals), when both are known.
    pub delta: Option<f64>,
}

/// A grade on both sides.
#[derive(Clone, Debug, Serialize)]
pub struct GradeDelta {
    pub name: &'static str,
    pub old: String,
    pub new: String,
    pub old_value: f64,
    pub new_value: f64,
}

/// The comparison of two results.
#[derive(Clone, Debug, Serialize)]
pub struct Comparison {
    pub compare_version: u32,
    pub old: Side,
    pub new: Side,
    /// Why some differences may not come from code changes.
    pub warnings: Vec<String>,
    pub counts: Counts,
    pub added: Vec<Item>,
    pub fixed: Vec<Item>,
    pub severity_changed: Vec<Item>,
    pub newly_suppressed: Vec<Item>,
    pub unsuppressed: Vec<Item>,
    pub measures: Vec<MeasureDelta>,
    pub grades: Vec<GradeDelta>,
}

fn side(file: &str, r: &ScanResult) -> Side {
    Side {
        file: file.replace('\\', "/"),
        engine_version: r.engine_version.clone(),
        format_version: r.format_version,
        kind: r.source.kind.clone(),
        rev: r.source.rev.clone(),
        commit: r.source.commit.clone(),
        findings: r.active_findings().count() as u64,
        gate_passed: r.gate.passed,
    }
}

fn grades_of(r: &ScanResult) -> Grades {
    r.grades.clone().unwrap_or_else(|| {
        grades::compute(
            &r.measures,
            r.new_code.as_ref(),
            &mote_engine::config::GradesConfig::default(),
        )
    })
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn measures(old: &ScanResult, new: &ScanResult) -> Vec<MeasureDelta> {
    type Get = fn(&ScanResult) -> Option<f64>;
    let rows: [(&'static str, &'static str, Get); 15] = [
        ("files", "Files", |r| Some(r.measures.total.files as f64)),
        ("lines", "Lines", |r| Some(r.measures.total.lines as f64)),
        ("ncloc", "Lines of code", |r| {
            Some(r.measures.total.ncloc as f64)
        }),
        ("functions", "Functions", |r| {
            Some(r.measures.total.functions as f64)
        }),
        ("cognitive_complexity", "Cognitive complexity", |r| {
            Some(r.measures.total.cognitive_complexity as f64)
        }),
        ("cyclomatic_complexity", "Cyclomatic complexity", |r| {
            Some(r.measures.total.cyclomatic_complexity as f64)
        }),
        ("duplicated_lines", "Duplicated lines", |r| {
            Some(r.measures.total.duplicated_lines as f64)
        }),
        ("duplicated_lines_density", "Duplication %", |r| {
            Some(r.measures.total.duplicated_lines_density)
        }),
        ("coverage_pct", "Coverage %", |r| {
            r.measures.coverage.as_ref().map(|c| c.coverage_pct)
        }),
        ("findings", "Findings", |r| {
            Some(r.measures.total.findings as f64)
        }),
        ("findings_high", "HIGH findings", |r| {
            Some(r.measures.total.findings_high as f64)
        }),
        ("findings_medium", "MEDIUM findings", |r| {
            Some(r.measures.total.findings_medium as f64)
        }),
        ("findings_low", "LOW findings", |r| {
            Some(r.measures.total.findings_low as f64)
        }),
        ("suppressed_findings", "Suppressed findings", |r| {
            Some(r.measures.total.suppressed_findings as f64)
        }),
        ("effort_minutes", "Estimated effort (min)", |r| {
            Some(r.measures.total.effort_minutes.total as f64)
        }),
    ];
    rows.iter()
        .map(|&(name, label, get)| {
            let (o, n) = (get(old), get(new));
            MeasureDelta {
                name,
                label,
                old: o,
                new: n,
                delta: o.zip(n).map(|(o, n)| round2(n - o)),
            }
        })
        .collect()
}

/// Compare `old` with `new` (`old_file`, `new_file` name them in the output).
pub fn compare(old_file: &str, old: &ScanResult, new_file: &str, new: &ScanResult) -> Comparison {
    let mut warnings = Vec::new();
    if old.engine_version != new.engine_version {
        warnings.push(format!(
            "the results come from different engine versions ({} and {}): rule changes show up as added or fixed findings",
            old.engine_version, new.engine_version
        ));
    }
    if old.format_version != new.format_version {
        warnings.push(format!(
            "the results have different formats (version {} and {}): fields missing from the older one count as empty",
            old.format_version, new.format_version
        ));
    }
    let old_rules: Vec<String> = old.rules.iter().map(|r| r.id.clone()).collect();
    if let Some(d) = rule_set_difference(&old_rules, new.rules.iter().map(|r| r.id.as_str())) {
        warnings.push(format!(
            "the scans used different rules ({d}): their findings show up as added or fixed"
        ));
    }
    let (og, ng) = (grades_of(old), grades_of(new));
    if og.bands != ng.bands || og.grades_version != ng.grades_version {
        warnings.push(
            "the grades were computed with different bands or methods; compare their values with care"
                .into(),
        );
    }
    if old.source.prefix != new.source.prefix {
        warnings.push(format!(
            "the scans have different roots in the repository (`{}` and `{}`): paths differ",
            old.source.prefix, new.source.prefix
        ));
    }

    let o: Vec<&Finding> = old.findings.iter().collect();
    let n: Vec<&Finding> = new.findings.iter().collect();
    let of: FxHashSet<&str> = old.files.iter().map(|f| f.path.as_str()).collect();
    let nf: FxHashSet<&str> = new.files.iter().map(|f| f.path.as_str()).collect();
    let pairs = matching::match_findings(&o, &n, Files { old: &of, new: &nf });

    let mut c = Comparison {
        compare_version: COMPARE_VERSION,
        old: side(old_file, old),
        new: side(new_file, new),
        warnings,
        counts: Counts::default(),
        added: Vec::new(),
        fixed: Vec::new(),
        severity_changed: Vec::new(),
        newly_suppressed: Vec::new(),
        unsuppressed: Vec::new(),
        measures: measures(old, new),
        grades: vec![
            GradeDelta {
                name: "maintainability",
                old: og.maintainability.grade.as_str().into(),
                new: ng.maintainability.grade.as_str().into(),
                old_value: og.maintainability.value,
                new_value: ng.maintainability.value,
            },
            GradeDelta {
                name: "reliability",
                old: og.reliability.grade.as_str().into(),
                new: ng.reliability.grade.as_str().into(),
                old_value: og.reliability.value,
                new_value: ng.reliability.value,
            },
        ],
    };
    let mut used = vec![false; o.len()];
    for (f, m) in n.iter().zip(&pairs) {
        let Some(i) = *m else {
            if f.suppressed.is_none() {
                c.added.push(Item::of(f));
            }
            continue;
        };
        used[i] = true;
        let before = o[i];
        let mut item = Item::of(f);
        if before.path != f.path {
            item.old_path = Some(before.path.clone());
        }
        match (before.suppressed.is_some(), f.suppressed.is_some()) {
            (false, false) if before.severity != f.severity => {
                item.old_severity = Some(before.severity);
                c.severity_changed.push(item);
            }
            (false, false) => c.counts.unchanged += 1,
            (false, true) => c.newly_suppressed.push(item),
            (true, false) => c.unsuppressed.push(item),
            (true, true) => {}
        }
    }
    for (f, u) in o.iter().zip(&used) {
        if !u && f.suppressed.is_none() {
            c.fixed.push(Item::of(f));
        }
    }
    c.counts.added = c.added.len() as u64;
    c.counts.fixed = c.fixed.len() as u64;
    c.counts.severity_changed = c.severity_changed.len() as u64;
    c.counts.newly_suppressed = c.newly_suppressed.len() as u64;
    c.counts.unsuppressed = c.unsuppressed.len() as u64;
    c
}

/// Options of the text and Markdown renderings.
#[derive(Clone, Copy, Debug)]
pub struct CompareOptions {
    /// Findings listed per section (the JSON lists all).
    pub max_listed: usize,
}

impl Default for CompareOptions {
    fn default() -> Self {
        CompareOptions { max_listed: 50 }
    }
}

/// The comparison as JSON (every finding listed).
pub fn to_json(c: &Comparison) -> Value {
    serde_json::to_value(c).unwrap_or_else(|_| json!({}))
}

fn describe(s: &Side) -> String {
    let mut parts = Vec::new();
    if let Some(rev) = &s.rev {
        parts.push(rev.clone());
    }
    if let Some(c) = &s.commit {
        parts.push(format!("commit {}", short_commit(c)));
    }
    parts.push(format!("mote {}", s.engine_version));
    format!("{} ({})", s.file, parts.join(", "))
}

fn fmt_value(name: &str, v: Option<f64>) -> String {
    match v {
        None => "–".into(),
        Some(v) if name.ends_with("_density") || name.ends_with("_pct") => format!("{v:.1}%"),
        Some(v) if name == "effort_minutes" => fmt_hours(v.max(0.0) as u64),
        Some(v) => thousands(v.max(0.0) as u64),
    }
}

fn fmt_delta(name: &str, d: Option<f64>) -> String {
    match d {
        None => "–".into(),
        Some(0.0) => "0".into(),
        Some(d) => {
            let sign = if d > 0.0 { "+" } else { "−" };
            let a = d.abs();
            let body = if name.ends_with("_density") || name.ends_with("_pct") {
                format!("{a:.1} pts")
            } else if name == "effort_minutes" {
                fmt_hours(a as u64)
            } else {
                thousands(a as u64)
            };
            format!("{sign}{body}")
        }
    }
}

fn summary_line(c: &Comparison) -> String {
    let k = &c.counts;
    format!(
        "{} added, {} fixed, {} with another severity, {} newly suppressed, {} no longer suppressed, {} unchanged",
        thousands(k.added),
        thousands(k.fixed),
        thousands(k.severity_changed),
        thousands(k.newly_suppressed),
        thousands(k.unsuppressed),
        thousands(k.unchanged)
    )
}

fn sections(c: &Comparison) -> [(&'static str, &Vec<Item>); 5] {
    [
        ("Added", &c.added),
        ("Fixed", &c.fixed),
        ("Severity changed", &c.severity_changed),
        ("Newly suppressed", &c.newly_suppressed),
        ("No longer suppressed", &c.unsuppressed),
    ]
}

fn severity_of(i: &Item) -> String {
    match i.old_severity {
        Some(o) => format!("{} → {}", o.as_str(), i.severity.as_str()),
        None => i.severity.as_str().to_string(),
    }
}

fn location(i: &Item) -> String {
    match &i.old_path {
        Some(p) => format!("{}:{} (was {p})", i.path, i.line),
        None => format!("{}:{}", i.path, i.line),
    }
}

/// Plain-text rendering.
pub fn render_text(c: &Comparison, opts: &CompareOptions) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "Old: {}", describe(&c.old));
    let _ = writeln!(s, "New: {}", describe(&c.new));
    for w in &c.warnings {
        let _ = writeln!(s, "warning: {w}");
    }
    let _ = writeln!(s, "\nFindings: {}", summary_line(c));
    let gate = |p: bool| if p { "passed" } else { "failed" };
    let _ = writeln!(
        s,
        "Quality gate: {} → {}",
        gate(c.old.gate_passed),
        gate(c.new.gate_passed)
    );
    let _ = writeln!(
        s,
        "\n{:<26}{:>12}{:>12}{:>12}",
        "Measure", "old", "new", "change"
    );
    for m in &c.measures {
        let _ = writeln!(
            s,
            "{:<26}{:>12}{:>12}{:>12}",
            m.label,
            fmt_value(m.name, m.old),
            fmt_value(m.name, m.new),
            fmt_delta(m.name, m.delta)
        );
    }
    for g in &c.grades {
        let _ = writeln!(
            s,
            "{:<26}{:>12}{:>12}",
            format!("{} grade", capitalize(g.name)),
            format!("{} ({:.2})", g.old, g.old_value),
            format!("{} ({:.2})", g.new, g.new_value)
        );
    }
    for (title, list) in sections(c) {
        if list.is_empty() {
            continue;
        }
        let _ = writeln!(s, "\n{title} ({})", thousands(list.len() as u64));
        for i in list.iter().take(opts.max_listed) {
            let _ = writeln!(
                s,
                "  {:<8} {}  {}  {}",
                severity_of(i),
                location(i),
                i.rule,
                i.message
            );
        }
        if list.len() > opts.max_listed {
            let _ = writeln!(
                s,
                "  … and {} more (see --format json)",
                thousands((list.len() - opts.max_listed) as u64)
            );
        }
    }
    s
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Markdown rendering (a CI job summary or a merge-request note).
pub fn render_markdown(c: &Comparison, opts: &CompareOptions) -> String {
    let esc = crate::markdown::cell;
    let code = crate::markdown::code;
    let mut s = String::new();
    let _ = writeln!(s, "## Mote: comparison\n");
    let _ = writeln!(
        s,
        "Old: {}  \nNew: {}\n",
        esc(&describe(&c.old)),
        esc(&describe(&c.new))
    );
    for w in &c.warnings {
        let _ = writeln!(s, "> :warning: {}\n", esc(w));
    }
    let _ = writeln!(s, "**Findings:** {}.\n", summary_line(c));
    let _ = writeln!(s, "| Measure | Old | New | Change |");
    let _ = writeln!(s, "|---|---:|---:|---:|");
    for m in &c.measures {
        let _ = writeln!(
            s,
            "| {} | {} | {} | {} |",
            m.label,
            fmt_value(m.name, m.old),
            fmt_value(m.name, m.new),
            fmt_delta(m.name, m.delta)
        );
    }
    for g in &c.grades {
        let _ = writeln!(
            s,
            "| {} grade | **{}** ({:.2}) | **{}** ({:.2}) | |",
            capitalize(g.name),
            g.old,
            g.old_value,
            g.new,
            g.new_value
        );
    }
    let gate = |p: bool| if p { "passed" } else { "failed" };
    let _ = writeln!(
        s,
        "| Quality gate | {} | {} | |",
        gate(c.old.gate_passed),
        gate(c.new.gate_passed)
    );
    for (title, list) in sections(c) {
        if list.is_empty() {
            continue;
        }
        let _ = writeln!(s, "\n### {title} ({})\n", thousands(list.len() as u64));
        let _ = writeln!(s, "| Severity | Location | Rule | Message |");
        let _ = writeln!(s, "|---|---|---|---|");
        for i in list.iter().take(opts.max_listed) {
            let _ = writeln!(
                s,
                "| {} | {} | {} | {} |",
                severity_of(i),
                code(&location(i)),
                code(&i.rule),
                esc(&i.message)
            );
        }
        if list.len() > opts.max_listed {
            let _ = writeln!(
                s,
                "\n_… and {} more; see the JSON comparison._",
                thousands((list.len() - opts.max_listed) as u64)
            );
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdata::{with_base, without_base};

    fn pair() -> (ScanResult, ScanResult) {
        let old = without_base();
        let mut new = without_base();
        // One finding fixed, one added, one severity changed, one renamed file.
        let fixed = new.findings.remove(0);
        let mut added = fixed.clone();
        added.fingerprint = "brand-new".into();
        added.id = "added0000001".into();
        added.path = "src/z/New.java".into();
        new.findings.push(added);
        let target = new
            .findings
            .iter_mut()
            .find(|f| f.suppressed.is_none() && f.severity != Severity::High)
            .unwrap();
        target.severity = Severity::High;
        (old, new)
    }

    #[test]
    fn lists_and_counts() {
        let (old, new) = pair();
        let c = compare("old.json", &old, "new.json", &new);
        assert_eq!(c.counts.added, 1);
        assert_eq!(c.added[0].path, "src/z/New.java");
        assert_eq!(c.counts.fixed, 1);
        assert_eq!(c.counts.severity_changed, 1);
        assert_eq!(c.severity_changed[0].severity, Severity::High);
        assert!(c.severity_changed[0].old_severity.is_some());
        assert!(c.warnings.is_empty(), "{:?}", c.warnings);
        let same = compare("a", &old, "b", &old);
        assert_eq!(same.counts.added + same.counts.fixed, 0);
        assert!(same
            .measures
            .iter()
            .all(|m| m.delta.is_none_or(|d| d == 0.0)));
        let v = to_json(&c);
        assert_eq!(v["compare_version"], 1);
        assert_eq!(v["counts"]["added"], 1);
        assert_eq!(v["added"][0]["id"], "added0000001");
        assert!(v["added"][0].get("old_severity").is_none());
    }

    #[test]
    fn suppression_changes_and_renames() {
        let old = without_base();
        let mut new = without_base();
        let sup_idx = new
            .findings
            .iter()
            .position(|f| f.suppressed.is_some())
            .unwrap();
        new.findings[sup_idx].suppressed = None; // no longer suppressed
        let active = new
            .findings
            .iter()
            .position(|f| f.suppressed.is_none() && f.path == "src/c/Util.java")
            .unwrap();
        new.findings[active].suppressed = Some(mote_core::SuppressionInfo {
            kind: "accepted".into(),
            justification: Some("kept".into()),
        });
        // src/b/Legacy.java renamed to src/b/Legacy2.java.
        for f in &mut new.findings {
            if f.path == "src/b/Legacy.java" {
                f.path = "src/b/Legacy2.java".into();
            }
        }
        for f in &mut new.files {
            if f.path == "src/b/Legacy.java" {
                f.path = "src/b/Legacy2.java".into();
            }
        }
        let c = compare("old.json", &old, "new.json", &new);
        assert_eq!(c.counts.unsuppressed, 1, "{c:#?}");
        assert_eq!(c.counts.newly_suppressed, 1);
        assert_eq!((c.counts.added, c.counts.fixed), (0, 0), "{c:#?}");
    }

    #[test]
    fn warnings_and_renderings() {
        let old = without_base();
        let mut new = with_base();
        new.engine_version = "0.2.0".into();
        new.rules.pop();
        let c = compare("old.json", &old, "new.json", &new);
        assert!(c.warnings.iter().any(|w| w.contains("0.1.0 and 0.2.0")));
        assert!(c.warnings.iter().any(|w| w.contains("no longer enabled")));
        let t = render_text(&c, &CompareOptions::default());
        assert!(
            t.starts_with(
                "Old: old.json (mote 0.1.0)\nNew: new.json (commit 0123456789ab, mote 0.2.0)"
            ),
            "{t}"
        );
        assert!(t.contains("warning: the results come from different engine versions"));
        assert!(t.contains("Lines of code"));
        assert!(t.contains("Maintainability grade"));
        let md = render_markdown(&c, &CompareOptions { max_listed: 0 });
        assert!(md.starts_with("## Mote: comparison"));
        assert!(md.contains("| Measure | Old | New | Change |"));
        assert!(md.contains(":warning:"));
        let (o, n) = pair();
        let c = compare("o", &o, "n", &n);
        let md = render_markdown(&c, &CompareOptions { max_listed: 0 });
        assert!(md.contains("### Added (1)"), "{md}");
        assert!(md.contains("_… and 1 more"), "{md}");
        let t = render_text(&c, &CompareOptions::default());
        assert!(t.contains("Added (1)\n  HIGH"), "{t}");
    }
}
