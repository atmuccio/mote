//! Self-contained HTML report: one file, no network.
//!
//! The page embeds its stylesheet and script inline and the report data as JSON in a
//! `<script type="application/json">` block (not executed; `<`, `>`, `&`, U+2028 and U+2029 are
//! written as `\u` escapes, so no message or path can close the block or open markup). A
//! Content-Security-Policy meta tag allows exactly that script and stylesheet, by their SHA-256
//! hashes, and nothing else: no other script, no network connections, no frames, no forms. The
//! script builds every element with `textContent`, never from HTML strings.
//!
//! Views: an overview (quality gate, grades, measures, findings by severity, quality and rule,
//! new code against overall), the files as a directory tree with each file's findings (no source
//! code is embedded), and the rules with their descriptions (shown as plain text). The report
//! lists at most [`HtmlOptions::max_findings`] findings (the most severe, like SARIF) and says so
//! on the page; every count and measure covers all findings. Suppressed findings are counted,
//! not listed. Paths are relative to the scan root, like the text and JSON output.

use crate::sha256::csp_hash;
use crate::{
    condition_label, fmt_condition_threshold, fmt_condition_value, most_severe, short_commit,
    VERSION,
};
use mote_core::{Finding, FindingStatus, Quality, Severity};
use mote_engine::grades::Grades;
use mote_engine::measures::Measures;
use mote_engine::result::{
    CoverageSummary, Diagnostic, FileStatus, NewCodeMeasures, SeverityCounts,
};
use mote_engine::source::SourceInfo;
use mote_engine::ScanResult;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::io::{self, Write};

/// Default [`HtmlOptions::max_findings`] (the SARIF limit).
pub const MAX_FINDINGS: usize = 25_000;

/// Most diagnostics listed on the page.
const MAX_DIAGNOSTICS: usize = 100;

const SCRIPT: &str = include_str!("html/report.js");
const STYLE: &str = include_str!("html/report.css");

/// HTML options.
#[derive(Clone, Copy, Debug)]
pub struct HtmlOptions {
    /// Most findings listed; the most severe are kept.
    pub max_findings: usize,
}

impl Default for HtmlOptions {
    fn default() -> Self {
        HtmlOptions {
            max_findings: MAX_FINDINGS,
        }
    }
}

#[derive(Serialize)]
struct View<'a> {
    tool: &'static str,
    version: &'static str,
    format_version: u32,
    engine_version: &'a str,
    source: &'a SourceInfo,
    base: Option<Base<'a>>,
    gate: Gate,
    grades: Option<&'a Grades>,
    measures: MeasuresView<'a>,
    counts: Counts,
    rules: Vec<RuleView<'a>>,
    /// `[path, language, is_test, ncloc, [high, medium, low], effort_minutes, change]`
    files: Vec<FileRow<'a>>,
    skipped_files: u64,
    /// Distinct finding messages (findings refer to them by index).
    messages: Vec<&'a str>,
    /// `[file, rule, line, column, severity (0 HIGH, 1 MEDIUM, 2 LOW), status, message]`;
    /// status 0 no base, 1 new, 2 unchanged, 3 worsened.
    findings: Vec<FindingRow>,
    /// Unsuppressed findings in the result.
    total_findings: usize,
    /// Of those, left out of `findings`.
    omitted_findings: usize,
    max_findings: usize,
    diagnostics: Vec<&'a Diagnostic>,
    diagnostics_total: usize,
}

#[derive(Serialize)]
struct Base<'a> {
    base: &'a str,
    merge_base: &'a str,
    /// The comparison was with a baseline results file (no line diff).
    baseline: bool,
}

#[derive(Serialize)]
struct Gate {
    passed: bool,
    conditions: Vec<ConditionView>,
}

#[derive(Serialize)]
struct ConditionView {
    label: String,
    /// `pass`, `fail` or `skip`.
    result: &'static str,
    actual: String,
    threshold: String,
    reason: Option<String>,
}

#[derive(Serialize)]
struct MeasuresView<'a> {
    total: &'a Measures,
    main: &'a Measures,
    test: &'a Measures,
    languages: &'a BTreeMap<String, Measures>,
    coverage: Option<&'a CoverageSummary>,
    new_code: Option<&'a NewCodeMeasures>,
}

/// Counts by severity (HIGH, MEDIUM, LOW) over all unsuppressed findings.
#[derive(Serialize, Default)]
struct Counts {
    total: [u64; 3],
    new: [u64; 3],
    worsened: [u64; 3],
    fixed: [u64; 3],
    reliability: [u64; 3],
    maintainability: [u64; 3],
    incubating: u64,
    suppressed: u64,
}

#[derive(Serialize)]
struct RuleView<'a> {
    id: &'a str,
    name: &'a str,
    language: &'a str,
    severity: &'static str,
    quality: &'static str,
    status: &'static str,
    tags: &'a [String],
    remediation_minutes: u32,
    description: &'a str,
    /// `[name, default, description]`
    params: Vec<(&'a str, String, &'a str)>,
    /// Unsuppressed findings of the rule.
    count: u64,
    /// Of those, new or worsened.
    new: u64,
}

type FileRow<'a> = (
    &'a str,
    &'a str,
    bool,
    u32,
    [u32; 3],
    u64,
    Option<&'static str>,
);
type FindingRow = (usize, usize, u32, u32, u8, u8, usize);

fn sev(s: Severity) -> usize {
    match s {
        Severity::High => 0,
        Severity::Medium => 1,
        Severity::Low => 2,
    }
}

fn counts3(c: &SeverityCounts) -> [u64; 3] {
    [c.high.into(), c.medium.into(), c.low.into()]
}

fn status(s: FindingStatus) -> u8 {
    match s {
        FindingStatus::Unknown => 0,
        FindingStatus::New => 1,
        FindingStatus::Unchanged => 2,
        FindingStatus::Worsened => 3,
    }
}

fn gate(result: &ScanResult) -> Gate {
    Gate {
        passed: result.gate.passed,
        conditions: result
            .gate
            .conditions
            .iter()
            .map(|c| ConditionView {
                label: condition_label(&c.name),
                result: if c.skipped_reason.is_some() {
                    "skip"
                } else if c.passed {
                    "pass"
                } else {
                    "fail"
                },
                actual: c
                    .actual
                    .map(|v| fmt_condition_value(&c.name, v))
                    .unwrap_or_default(),
                threshold: c
                    .threshold
                    .map(|v| fmt_condition_threshold(&c.name, v))
                    .unwrap_or_default(),
                reason: c.skipped_reason.clone(),
            })
            .collect(),
    }
}

fn view<'a>(result: &'a ScanResult, opts: &HtmlOptions) -> View<'a> {
    let active: Vec<&Finding> = result.active_findings().collect();
    let mut counts = Counts {
        suppressed: (result.findings.len() - active.len()) as u64,
        ..Default::default()
    };
    if let Some(nc) = &result.new_code {
        counts.fixed = counts3(&nc.measures.fixed_findings);
    }

    // Rules: the catalog of the scan, plus any rule a finding names that it lacks.
    let mut rules: Vec<RuleView> = result
        .rules
        .iter()
        .map(|r| RuleView {
            id: &r.id,
            name: &r.name,
            language: &r.language,
            severity: r.severity.as_str(),
            quality: r.quality.as_str(),
            status: match r.status {
                mote_core::RuleStatus::Stable => "stable",
                mote_core::RuleStatus::Incubating => "incubating",
                mote_core::RuleStatus::Deprecated => "deprecated",
            },
            tags: &r.tags,
            remediation_minutes: r.remediation_minutes,
            description: &r.description,
            params: r
                .params
                .iter()
                .map(|p| {
                    (
                        p.name.as_str(),
                        crate::rules::param_value(&p.default),
                        p.description.as_str(),
                    )
                })
                .collect(),
            count: 0,
            new: 0,
        })
        .collect();
    let mut rule_index: HashMap<&str, usize> = HashMap::new();
    for (i, r) in rules.iter().enumerate() {
        rule_index.entry(r.id).or_insert(i);
    }
    for f in &active {
        if !rule_index.contains_key(f.rule_id.as_str()) {
            rule_index.insert(&f.rule_id, rules.len());
            rules.push(RuleView {
                id: &f.rule_id,
                name: &f.rule_id,
                language: f.rule_id.split('/').next().unwrap_or(""),
                severity: f.severity.as_str(),
                quality: f.quality.as_str(),
                status: "stable",
                tags: &[],
                remediation_minutes: 0,
                description: "",
                params: Vec::new(),
                count: 0,
                new: 0,
            });
        }
    }

    // Files: every analyzed file, plus any path a finding names that the list lacks.
    let mut files: BTreeMap<&str, FileRow> = BTreeMap::new();
    let mut skipped_files = 0u64;
    for f in &result.files {
        if f.status == FileStatus::Skipped {
            skipped_files += 1;
            continue;
        }
        files.insert(
            &f.path,
            (
                &f.path,
                &f.language,
                f.is_test,
                f.measures.as_ref().map_or(0, |m| m.ncloc),
                [0; 3],
                f.effort_minutes,
                f.change.map(|c| match c {
                    mote_engine::newcode::ChangeKind::Added => "added",
                    mote_engine::newcode::ChangeKind::Modified => "modified",
                    mote_engine::newcode::ChangeKind::Renamed => "renamed",
                    mote_engine::newcode::ChangeKind::Copied => "copied",
                    mote_engine::newcode::ChangeKind::Untracked => "untracked",
                }),
            ),
        );
    }
    for f in &active {
        let row = files
            .entry(&f.path)
            .or_insert((&f.path, "", false, 0, [0; 3], 0, None));
        row.4[sev(f.severity)] += 1;
        let i = sev(f.severity);
        counts.total[i] += 1;
        match f.status {
            FindingStatus::New => counts.new[i] += 1,
            FindingStatus::Worsened => counts.worsened[i] += 1,
            _ => {}
        }
        match f.quality {
            Quality::Reliability => counts.reliability[i] += 1,
            Quality::Maintainability => counts.maintainability[i] += 1,
        }
        counts.incubating += u64::from(f.incubating);
        let r = &mut rules[rule_index[f.rule_id.as_str()]];
        r.count += 1;
        if matches!(f.status, FindingStatus::New | FindingStatus::Worsened) {
            r.new += 1;
        }
    }
    let file_index: HashMap<&str, usize> = files.keys().enumerate().map(|(i, p)| (*p, i)).collect();

    let total_findings = active.len();
    let kept = most_severe(active, opts.max_findings);
    let mut messages: Vec<&str> = Vec::new();
    let mut message_index: HashMap<&str, usize> = HashMap::new();
    let findings: Vec<FindingRow> = kept
        .iter()
        .map(|f| {
            let m = *message_index.entry(&f.message).or_insert_with(|| {
                messages.push(&f.message);
                messages.len() - 1
            });
            (
                file_index[f.path.as_str()],
                rule_index[f.rule_id.as_str()],
                f.range.start_line,
                f.range.start_col,
                sev(f.severity) as u8,
                status(f.status),
                m,
            )
        })
        .collect();

    View {
        tool: "Mote",
        version: VERSION,
        format_version: result.format_version,
        engine_version: &result.engine_version,
        source: &result.source,
        base: result.new_code.as_ref().map(|nc| Base {
            base: &nc.base,
            merge_base: short_commit(&nc.merge_base),
            baseline: nc.baseline.is_some(),
        }),
        gate: gate(result),
        grades: result.grades.as_ref(),
        measures: MeasuresView {
            total: &result.measures.total,
            main: &result.measures.main,
            test: &result.measures.test,
            languages: &result.measures.languages,
            coverage: result.measures.coverage.as_ref(),
            new_code: result.new_code.as_ref().map(|nc| &nc.measures),
        },
        counts,
        rules,
        files: files.into_values().collect(),
        skipped_files,
        messages,
        omitted_findings: total_findings - findings.len(),
        findings,
        total_findings,
        max_findings: opts.max_findings,
        diagnostics: result.diagnostics.iter().take(MAX_DIAGNOSTICS).collect(),
        diagnostics_total: result.diagnostics.len(),
    }
}

/// JSON for a `<script type="application/json">` block: `<`, `>`, `&` and the JavaScript line
/// terminators U+2028 / U+2029 as `\u` escapes (they can only occur inside JSON strings, where
/// the escapes mean the same), so the data can never end the block or start markup.
pub fn script_safe_json(json: &str) -> String {
    let mut out = String::with_capacity(json.len() + json.len() / 16);
    for c in json.chars() {
        match c {
            '<' => out.push_str("\\u003c"),
            '>' => out.push_str("\\u003e"),
            '&' => out.push_str("\\u0026"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c => out.push(c),
        }
    }
    out
}

/// The Content-Security-Policy of the page: only the embedded script and stylesheet run.
pub fn content_security_policy() -> String {
    format!(
        "default-src 'none'; script-src {}; style-src {}; img-src data:; base-uri 'none'; form-action 'none'",
        csp_hash(SCRIPT),
        csp_hash(STYLE)
    )
}

/// Render the page.
pub fn render(result: &ScanResult, opts: &HtmlOptions) -> String {
    let data = serde_json::to_string(&view(result, opts)).unwrap_or_else(|_| "{}".into());
    let data = script_safe_json(&data);
    let mut out = String::with_capacity(data.len() + SCRIPT.len() + STYLE.len() + 1024);
    out.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    out.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    out.push_str("<meta http-equiv=\"Content-Security-Policy\" content=\"");
    out.push_str(&content_security_policy());
    out.push_str("\">\n<meta name=\"referrer\" content=\"no-referrer\">\n");
    out.push_str("<meta name=\"generator\" content=\"Mote ");
    out.push_str(VERSION);
    out.push_str("\">\n<title>Mote report</title>\n<style>");
    out.push_str(STYLE);
    out.push_str("</style>\n</head>\n<body>\n<noscript><p>This report needs JavaScript to display its data.</p></noscript>\n");
    out.push_str("<script id=\"mote-data\" type=\"application/json\">");
    out.push_str(&data);
    out.push_str("</script>\n<script>");
    out.push_str(SCRIPT);
    out.push_str("</script>\n</body>\n</html>\n");
    out
}

/// How many findings the page leaves out to stay within `max_findings`.
pub fn omitted_findings(result: &ScanResult, opts: &HtmlOptions) -> usize {
    result
        .active_findings()
        .count()
        .saturating_sub(opts.max_findings)
}

/// Write the page.
pub fn write(result: &ScanResult, opts: &HtmlOptions, w: &mut dyn Write) -> io::Result<()> {
    w.write_all(render(result, opts).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdata::{empty, finding, with_base, without_base};
    use serde_json::Value;

    /// The embedded data, parsed back.
    fn data(html: &str) -> Value {
        let open = "<script id=\"mote-data\" type=\"application/json\">";
        let start = html.find(open).unwrap() + open.len();
        let end = start + html[start..].find("</script>").unwrap();
        serde_json::from_str(&html[start..end]).unwrap()
    }

    fn between<'a>(html: &'a str, open: &str, close: &str) -> &'a str {
        let start = html.find(open).unwrap() + open.len();
        let end = start + html[start..].find(close).unwrap();
        &html[start..end]
    }

    #[test]
    fn structure_csp_and_data() {
        let html = render(&with_base(), &HtmlOptions::default());
        assert!(html.starts_with("<!DOCTYPE html>\n<html lang=\"en\">"));
        assert_eq!(html.matches("<script").count(), 2);
        assert_eq!(html.matches("</script>").count(), 2);
        // The CSP hashes are those of the exact inline script and style.
        let csp = content_security_policy();
        assert!(html.contains(&format!(
            "<meta http-equiv=\"Content-Security-Policy\" content=\"{csp}\">"
        )));
        assert!(csp.contains(&csp_hash(between(
            &html,
            "</script>\n<script>",
            "</script>"
        ))));
        assert!(csp.contains(&csp_hash(between(&html, "<style>", "</style>"))));
        assert!(csp.starts_with("default-src 'none'; script-src 'sha256-"));
        assert!(
            !html.contains("http://") && !html.contains("https://"),
            "no network"
        );
        let v = data(&html);
        assert_eq!(v["tool"], "Mote");
        assert_eq!(v["gate"]["passed"], false);
        assert_eq!(v["gate"]["conditions"][0]["label"], "New findings");
        assert_eq!(v["gate"]["conditions"][0]["threshold"], "<= 0");
        assert_eq!(v["gate"]["conditions"][3]["actual"], "E");
        assert_eq!(v["gate"]["conditions"][3]["threshold"], "B or better");
        assert_eq!(v["base"]["merge_base"], "fedcba987654");
        assert_eq!(v["base"]["baseline"], false);
        let b = data(&render(
            &crate::testdata::with_baseline(),
            &HtmlOptions::default(),
        ));
        assert_eq!(b["base"]["baseline"], true);
        assert_eq!(b["base"]["base"], "mote-baseline.json");
        assert_eq!(v["counts"]["total"], serde_json::json!([1, 3, 1]));
        assert_eq!(v["counts"]["new"], serde_json::json!([1, 1, 1]));
        assert_eq!(v["counts"]["suppressed"], 1);
        assert_eq!(v["total_findings"], 5);
        assert_eq!(v["omitted_findings"], 0);
        assert_eq!(v["findings"].as_array().unwrap().len(), 5);
        assert_eq!(v["files"].as_array().unwrap().len(), 3, "skipped left out");
        assert_eq!(v["skipped_files"], 1);
        assert_eq!(v["files"][0][0], "src/a/Order Service.java");
        assert_eq!(v["files"][0][4], serde_json::json!([1, 2, 0]));
        assert_eq!(v["grades"]["maintainability"]["grade"], "D");
        let rules = v["rules"].as_array().unwrap();
        let todo = rules
            .iter()
            .find(|r| r["id"] == "java/todo-comment")
            .unwrap();
        assert_eq!(
            (todo["count"].as_u64(), todo["new"].as_u64()),
            (Some(1), Some(1))
        );
        // The TODO message appears twice in the result (one suppressed): stored once.
        let msgs = v["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 5);
        for r in [without_base(), empty()] {
            let html = render(&r, &HtmlOptions::default());
            let v = data(&html);
            assert!(v["base"].is_null());
        }
    }

    #[test]
    fn a_small_change_is_not_graded() {
        let v = data(&render(
            &crate::testdata::small_change(),
            &HtmlOptions::default(),
        ));
        assert!(v["grades"]["new_maintainability"].is_null());
        assert_eq!(v["grades"]["new_not_rated"]["new_lines"], 11);
        assert_eq!(v["grades"]["new_not_rated"]["min_new_lines"], 100);
        // The page says so instead of leaving the new-code grades out without a word.
        assert!(SCRIPT.contains("gr.new_not_rated"));
        assert!(SCRIPT.contains("'not rated'"));
    }

    #[test]
    fn deterministic() {
        let a = render(&with_base(), &HtmlOptions::default());
        let b = render(&with_base(), &HtmlOptions::default());
        assert_eq!(a, b);
    }

    #[test]
    fn capped_keeping_the_most_severe() {
        let mut r = with_base();
        r.findings = (0..30)
            .map(|i| {
                finding(
                    &format!("id{i:010}"),
                    "java/todo-comment",
                    &format!("src/F{i:02}.java"),
                    (1, 1, 1, 5),
                    if i % 3 == 0 {
                        Severity::High
                    } else {
                        Severity::Low
                    },
                    FindingStatus::New,
                    "m",
                )
            })
            .collect();
        let v = data(&render(&r, &HtmlOptions { max_findings: 12 }));
        assert_eq!(v["total_findings"], 30);
        assert_eq!(v["omitted_findings"], 18);
        let f = v["findings"].as_array().unwrap();
        assert_eq!(f.len(), 12);
        assert_eq!(f.iter().filter(|x| x[4] == 0).count(), 10, "all HIGH kept");
        assert_eq!(
            v["counts"]["total"],
            serde_json::json!([10, 0, 20]),
            "counts cover all"
        );
    }

    /// Messages and paths quote scanned code: nothing in them may end the data block, open
    /// markup or reach the page as HTML.
    #[test]
    fn hostile_strings_stay_data() {
        let mut r = with_base();
        let msg = "</script><script>alert(1)</script><img src=x onerror=alert(2)> <!-- & \u{2028}\u{2029} ]]>";
        let path = "src/</SCRIPT><img src=x onerror=alert(3)>.java";
        r.findings[0].message = msg.into();
        r.findings[1].path = path.into();
        r.rules[0].description = "<b>bold</b></script>".into();
        r.diagnostics[0].message = "</script>".into();
        let html = render(&r, &HtmlOptions::default());
        if let Some(out) = std::env::var_os("MOTE_HTML_HOSTILE_OUT") {
            std::fs::write(out, &html).unwrap();
        }
        let lower = html.to_ascii_lowercase();
        assert_eq!(
            lower.matches("<script").count(),
            2,
            "only the two real blocks"
        );
        assert_eq!(lower.matches("</script").count(), 2);
        assert!(!lower.contains("<img") && !lower.contains("<!--") && !lower.contains("<b>"));
        assert!(!html.contains('\u{2028}') && !html.contains('\u{2029}'));
        let v = data(&html);
        let msgs: Vec<&str> = v["messages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m.as_str().unwrap())
            .collect();
        assert!(msgs.contains(&msg), "round-trips exactly");
        assert!(v["files"].as_array().unwrap().iter().any(|f| f[0] == path));
        assert_eq!(
            script_safe_json("\"a<b>&c\u{2028}\""),
            "\"a\\u003cb\\u003e\\u0026c\\u2028\""
        );
    }

    /// Render a saved results file (`MOTE_RESULTS_JSON`), graded with the default bands, to
    /// `MOTE_HTML_OUT` and print its size: a smoke test on real data and the size
    /// measurement in docs/cli.md.
    #[test]
    #[ignore]
    fn render_saved_results() {
        let Some(path) = std::env::var_os("MOTE_RESULTS_JSON") else {
            return;
        };
        let text = std::fs::read_to_string(&path).unwrap();
        let mut r: ScanResult = serde_json::from_str(&text).unwrap();
        r.grades = Some(mote_engine::grades::compute(
            &r.measures,
            r.new_code.as_ref(),
            &mote_engine::config::GradesConfig::default(),
        ));
        let html = render(&r, &HtmlOptions::default());
        let v = data(&html);
        eprintln!(
            "{} bytes; {} findings listed of {}; {} files; {} rules; {} distinct messages",
            html.len(),
            v["findings"].as_array().unwrap().len(),
            v["total_findings"],
            v["files"].as_array().unwrap().len(),
            v["rules"].as_array().unwrap().len(),
            v["messages"].as_array().unwrap().len()
        );
        if let Some(out) = std::env::var_os("MOTE_HTML_OUT") {
            std::fs::write(out, &html).unwrap();
        }
    }

    /// The script never parses data as HTML.
    #[test]
    fn script_builds_elements_from_text_only() {
        for forbidden in [
            "innerHTML",
            "outerHTML",
            "insertAdjacentHTML",
            "document.write",
            "eval(",
            "new Function",
            "setAttribute('style'",
            "fetch(",
            "XMLHttpRequest",
        ] {
            assert!(!SCRIPT.contains(forbidden), "{forbidden}");
        }
    }
}
