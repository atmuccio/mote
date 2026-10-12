//! Bitbucket Cloud Code Insights: a report and its annotations, as two JSON documents the CI job
//! uploads with the public Code Insights REST API (`PUT …/commit/{commit}/reports/{id}` with the
//! report, then `POST …/reports/{id}/annotations` with the annotations, at most 100 per request).
//! Mote only writes the files; it never talks to Bitbucket.
//!
//! * **Report** — `{title, details, external_id, reporter, report_type: "BUG", result: "PASSED" |
//!   "FAILED", data: [...]}`; `result` is the quality gate; `details` lists the failed
//!   conditions; `data` holds at most [`MAX_DATA`] typed values (findings, grades, duplication,
//!   coverage, estimated effort, lines of code).
//! * **Annotations** — an array of `{external_id, annotation_type, severity, path, line, summary,
//!   details}`: new and worsened findings when the scan was compared with a base or a baseline,
//!   every unsuppressed finding otherwise; the most severe first when there are more than
//!   [`MAX_ANNOTATIONS`] (the API's limit per report), kept in result order. `external_id` is the
//!   finding id, `annotation_type` comes from the rule (`BUG` for reliability rules, `VULNERABILITY`
//!   for rules tagged `security`, else `CODE_SMELL` — the values the API defines), severities map
//!   HIGH → `HIGH`, MEDIUM → `MEDIUM`, LOW → `LOW`. Paths are relative to the repository root
//!   ([`BitbucketOptions::path_prefix`]).

use crate::{
    condition_label, fmt_condition_threshold, fmt_condition_value, fmt_hours, has_base,
    is_new_like, most_severe, repo_path, truncate_bytes,
};
use mote_core::{Finding, Quality, Severity};
use mote_engine::ScanResult;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;

/// Most annotations one report can hold.
pub const MAX_ANNOTATIONS: usize = 1000;
/// Most data fields one report can hold.
pub const MAX_DATA: usize = 10;
/// Longest annotation `summary`, in bytes.
pub const MAX_SUMMARY: usize = 450;
/// Longest report or annotation `details`, in bytes.
pub const MAX_DETAILS: usize = 2000;

/// Options of the Code Insights files.
#[derive(Clone, Copy, Debug)]
pub struct BitbucketOptions<'a> {
    /// Repository-relative path of the scan root (`backend/`), prepended to annotation paths.
    pub path_prefix: &'a str,
    pub max_annotations: usize,
}

impl Default for BitbucketOptions<'_> {
    fn default() -> Self {
        BitbucketOptions {
            path_prefix: "",
            max_annotations: MAX_ANNOTATIONS,
        }
    }
}

/// One annotation.
#[derive(Debug, Serialize)]
pub struct Annotation {
    pub external_id: String,
    pub annotation_type: &'static str,
    pub severity: &'static str,
    pub path: String,
    pub line: u32,
    pub summary: String,
    pub details: String,
}

fn clip(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut t = truncate_bytes(s, max.saturating_sub('…'.len_utf8())).to_string();
    t.push('…');
    t
}

fn severity(s: Severity) -> &'static str {
    match s {
        Severity::High => "HIGH",
        Severity::Medium => "MEDIUM",
        Severity::Low => "LOW",
    }
}

/// The findings annotated: new-like ones with a base, all unsuppressed ones otherwise.
fn annotated<'a>(r: &'a ScanResult, opts: &BitbucketOptions<'_>) -> Vec<&'a Finding> {
    let base = has_base(r);
    let list: Vec<&Finding> = r
        .active_findings()
        .filter(|f| !base || is_new_like(f))
        .collect();
    most_severe(list, opts.max_annotations)
}

/// The annotations.
pub fn annotations(r: &ScanResult, opts: &BitbucketOptions<'_>) -> Vec<Annotation> {
    let security: HashMap<&str, bool> = r
        .rules
        .iter()
        .map(|x| (x.id.as_str(), x.tags.iter().any(|t| t == "security")))
        .collect();
    let names: HashMap<&str, &str> = r
        .rules
        .iter()
        .map(|x| (x.id.as_str(), x.name.as_str()))
        .collect();
    annotated(r, opts)
        .into_iter()
        .map(|f| {
            let kind = if security.get(f.rule_id.as_str()).copied().unwrap_or(false) {
                "VULNERABILITY"
            } else if f.quality == Quality::Reliability {
                "BUG"
            } else {
                // Bitbucket's own enum value for maintainability issues.
                "CODE_SMELL"
            };
            let mut details = format!(
                "{} ({}, {}, {})",
                f.rule_id,
                f.severity.as_str(),
                f.quality.as_str(),
                crate::status_str(f.status)
            );
            if let Some(name) = names.get(f.rule_id.as_str()) {
                details.push_str(&format!(": {name}"));
            }
            if f.incubating {
                details.push_str(" (incubating rule: never fails the gate)");
            }
            Annotation {
                external_id: f.id.clone(),
                annotation_type: kind,
                severity: severity(f.severity),
                path: repo_path(opts.path_prefix, &f.path).into_owned(),
                line: f.range.start_line.max(1),
                summary: clip(&f.message, MAX_SUMMARY),
                details: clip(&details, MAX_DETAILS),
            }
        })
        .collect()
}

/// The report.
pub fn report(r: &ScanResult, opts: &BitbucketOptions<'_>) -> Value {
    let base = has_base(r);
    let active: Vec<&Finding> = r.active_findings().collect();
    let counted: Vec<&&Finding> = active.iter().filter(|f| !base || is_new_like(f)).collect();
    let high = counted
        .iter()
        .filter(|f| f.severity == Severity::High)
        .count();
    let mut data: Vec<Value> = Vec::new();
    let mut push = |title: &str, kind: &str, value: Value| {
        data.push(json!({"title": title, "type": kind, "value": value}));
    };
    let (findings_title, high_title) = if base {
        ("New findings", "New HIGH findings")
    } else {
        ("Findings", "HIGH findings")
    };
    push(findings_title, "NUMBER", json!(counted.len()));
    push(high_title, "NUMBER", json!(high));
    if let Some(g) = &r.grades {
        push(
            "Maintainability grade",
            "TEXT",
            json!(g.maintainability.grade.as_str()),
        );
        push(
            "Reliability grade",
            "TEXT",
            json!(g.reliability.grade.as_str()),
        );
    }
    let nc = r.new_code.as_ref().filter(|nc| nc.baseline.is_none());
    match nc {
        Some(nc) => push(
            "Duplication on new code",
            "PERCENTAGE",
            json!(nc.measures.new_duplication_pct),
        ),
        None => push(
            "Duplication",
            "PERCENTAGE",
            json!(r.measures.total.duplicated_lines_density),
        ),
    }
    match (
        nc.and_then(|n| n.measures.new_coverage_pct),
        &r.measures.coverage,
    ) {
        (Some(p), _) => push("Coverage on new code", "PERCENTAGE", json!(p)),
        (None, Some(c)) => push("Coverage", "PERCENTAGE", json!(c.coverage_pct)),
        _ => {}
    }
    match &r.new_code {
        Some(nc) => push(
            "Estimated effort on new code",
            "TEXT",
            json!(fmt_hours(nc.measures.new_effort_minutes.total)),
        ),
        None => push(
            "Estimated effort",
            "TEXT",
            json!(fmt_hours(r.measures.total.effort_minutes.total)),
        ),
    }
    push("Lines of code", "NUMBER", json!(r.measures.total.ncloc));
    data.truncate(MAX_DATA);

    let mut details = if r.gate.passed {
        "Quality gate passed.".to_string()
    } else {
        let failed: Vec<String> = r
            .gate
            .conditions
            .iter()
            .filter(|c| !c.passed)
            .map(|c| {
                let actual = c
                    .actual
                    .map(|v| fmt_condition_value(&c.name, v))
                    .unwrap_or_default();
                let threshold = c
                    .threshold
                    .map(|v| format!(" ({})", fmt_condition_threshold(&c.name, v)))
                    .unwrap_or_default();
                format!("{} {actual}{threshold}", condition_label(&c.name))
            })
            .collect();
        format!("Quality gate failed: {}.", failed.join(", "))
    };
    match r.new_code.as_ref() {
        Some(nc) if nc.baseline.is_some() => {
            details.push_str(&format!(" Compared with the baseline {}.", nc.base))
        }
        Some(nc) => details.push_str(&format!(" Compared with {}.", nc.base)),
        None => details.push_str(" No base: every finding counts."),
    }
    let shown = annotated(r, opts).len();
    if shown < counted.len() {
        details.push_str(&format!(
            " {shown} of {} findings annotated (the most severe).",
            counted.len()
        ));
    }
    json!({
        "title": "Mote",
        "details": clip(&details, MAX_DETAILS),
        "external_id": "mote",
        "reporter": "Mote",
        "report_type": "BUG",
        "result": if r.gate.passed { "PASSED" } else { "FAILED" },
        "data": data,
    })
}

/// Both files, pretty-printed: `(report.json, annotations.json)`.
pub fn render(r: &ScanResult, opts: &BitbucketOptions<'_>) -> (String, String) {
    let mut report = serde_json::to_string_pretty(&report(r, opts)).unwrap_or_default();
    report.push('\n');
    let mut ann = serde_json::to_string_pretty(&annotations(r, opts)).unwrap_or_default();
    ann.push('\n');
    (report, ann)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdata::{assert_snapshot, with_base, with_baseline, without_base};

    /// The shape the Code Insights API accepts.
    fn validate(report: &Value, annotations: &Value) {
        for k in ["title", "details", "report_type", "result", "data"] {
            assert!(report.get(k).is_some(), "report needs `{k}`");
        }
        assert!(["SECURITY", "COVERAGE", "TEST", "BUG"]
            .contains(&report["report_type"].as_str().unwrap()));
        assert!(["PASSED", "FAILED", "PENDING"].contains(&report["result"].as_str().unwrap()));
        assert!(report["details"].as_str().unwrap().len() <= MAX_DETAILS);
        let data = report["data"].as_array().unwrap();
        assert!(data.len() <= MAX_DATA);
        for d in data {
            let t = d["type"].as_str().unwrap();
            assert!([
                "BOOLEAN",
                "DATE",
                "DURATION",
                "LINK",
                "NUMBER",
                "PERCENTAGE",
                "TEXT"
            ]
            .contains(&t));
            assert!(d["title"].is_string());
            match t {
                "NUMBER" | "PERCENTAGE" | "DURATION" => assert!(d["value"].is_number(), "{d}"),
                "TEXT" => assert!(d["value"].is_string(), "{d}"),
                _ => {}
            }
        }
        let list = annotations.as_array().unwrap();
        assert!(list.len() <= MAX_ANNOTATIONS);
        let mut ids = std::collections::HashSet::new();
        for a in list {
            assert!(
                ids.insert(a["external_id"].as_str().unwrap().to_string()),
                "unique ids"
            );
            assert!(["VULNERABILITY", "CODE_SMELL", "BUG"]
                .contains(&a["annotation_type"].as_str().unwrap()));
            assert!(
                ["CRITICAL", "HIGH", "MEDIUM", "LOW"].contains(&a["severity"].as_str().unwrap())
            );
            assert!(a["line"].as_u64().unwrap() >= 1);
            assert!(!a["path"].as_str().unwrap().is_empty());
            assert!(a["summary"].as_str().unwrap().len() <= MAX_SUMMARY);
            assert!(a["details"].as_str().unwrap().len() <= MAX_DETAILS);
        }
    }

    #[test]
    fn snapshots_and_shape() {
        let opts = BitbucketOptions {
            path_prefix: "backend/",
            ..Default::default()
        };
        let (report, ann) = render(&with_base(), &opts);
        assert_snapshot("bitbucket_report.json", &report);
        assert_snapshot("bitbucket_annotations.json", &ann);
        let (rv, av): (Value, Value) = (
            serde_json::from_str(&report).unwrap(),
            serde_json::from_str(&ann).unwrap(),
        );
        validate(&rv, &av);
        assert_eq!(rv["result"], "FAILED");
        assert!(rv["details"]
            .as_str()
            .unwrap()
            .starts_with("Quality gate failed: New findings 1 (<= 0)"));
        let list = av.as_array().unwrap();
        // New and worsened findings only, never the suppressed one or the unchanged one.
        let ids: Vec<&str> = list
            .iter()
            .map(|a| a["external_id"].as_str().unwrap())
            .collect();
        assert!(!ids.contains(&"sup000000001") && !ids.contains(&"unc000000001"));
        assert!(ids.contains(&"wrs000000001"));
        assert!(list
            .iter()
            .all(|a| a["path"].as_str().unwrap().starts_with("backend/src/")));
        let self_assign = list
            .iter()
            .find(|a| a["external_id"] == "new000000001")
            .unwrap();
        assert_eq!(self_assign["annotation_type"], "BUG");
        assert_eq!(self_assign["severity"], "HIGH");

        for r in [without_base(), with_baseline(), crate::testdata::empty()] {
            let (report, ann) = render(&r, &BitbucketOptions::default());
            validate(
                &serde_json::from_str(&report).unwrap(),
                &serde_json::from_str(&ann).unwrap(),
            );
        }
        let (report, _) = render(&without_base(), &BitbucketOptions::default());
        assert!(report.contains("No base: every finding counts."));
        let (report, _) = render(&with_baseline(), &BitbucketOptions::default());
        assert!(report.contains("Compared with the baseline mote-baseline.json."));
        assert!(
            !report.contains("Duplication on new code") && !report.contains("Coverage on new code"),
            "no line-based data: {report}"
        );
    }

    #[test]
    fn caps_and_long_messages() {
        let mut r = without_base();
        let template = r.findings[0].clone();
        r.findings.clear();
        for i in 0..1_200 {
            let mut f = template.clone();
            f.id = format!("id{i:010}");
            f.message = "x".repeat(600);
            f.severity = if i % 3 == 0 {
                Severity::High
            } else {
                Severity::Low
            };
            r.findings.push(f);
        }
        let a = annotations(&r, &BitbucketOptions::default());
        assert_eq!(a.len(), MAX_ANNOTATIONS);
        assert_eq!(
            a.iter().filter(|x| x.severity == "HIGH").count(),
            400,
            "every HIGH one kept"
        );
        assert!(a
            .iter()
            .all(|x| x.summary.len() <= MAX_SUMMARY && x.summary.ends_with('…')));
        let rep = report(&r, &BitbucketOptions::default());
        assert!(rep["details"]
            .as_str()
            .unwrap()
            .contains("1000 of 1200 findings annotated"));
    }
}
